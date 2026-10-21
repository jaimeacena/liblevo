//! Traducción local por bloques con Hy-MT2 (7B o 1,8B, GGUF, Apache-2.0) y `llama-server` de
//! llama.cpp b10566 como programa aparte, con huellas fijadas, igual que la relectura visual. El
//! servidor mantiene el modelo cargado durante todo el documento y solo escucha dentro del equipo.
//!
//! El modelo traduce un bloque de texto cada vez y nunca ve la estructura: las marcas de página,
//! las imágenes, el código y los prefijos de títulos y listas se conservan fuera; las tablas se
//! traducen celda a celda. El marcado en línea (énfasis, enlaces) se le pasa tal cual y se comprueba
//! que vuelve intacto; si no, se traduce el texto sin marcado y el bloque queda marcado para
//! revisar. Las guardas nunca reescriben: una traducción vacía, desproporcionada o sin traducir
//! deja el original o un aviso. El glosario se pasa con la plantilla de terminología del propio
//! modelo y se comprueba después.

use crate::visual_reader::PROGRAM_FILES as LLAMA_FILES;
use crate::xberg_reader::{ReaderError, sha256_file};
use liblevo_core::epub_package::EpubPackage;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::{Read, Seek};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, mpsc};
use std::time::{Duration, Instant};

/// Servidor de texto de llama.cpp b10566 y su biblioteca propia (el resto se comparte con la
/// relectura visual, salvo su programa: `mtmd.dll` también la carga el servidor).
const SERVER_FILES: [(&str, &str); 2] = [
    (
        "llama-server.exe",
        "fb645781ce81b28a4bfe479c66c6377ec9fba30e8c57705ff55e44fd3073282d",
    ),
    (
        "llama-server-impl.dll",
        "6f4701a79e655eb5ceadbdef26ceb0c19bb94082a4039f6adee4ce36a3b53b98",
    ),
];

/// Modelos Hy-MT2 (Q4_K_M) de `tencent/Hy-MT2-7B-GGUF` y `tencent/Hy-MT2-1.8B-GGUF`, revisiones
/// fijadas en el catálogo: (nombre, huella).
pub const MODEL_7B: (&str, &str) = (
    "Hy-MT2-7B-Q4_K_M.gguf",
    "9f96256500f3fc1ab4d64336b58f52a949a95ad7516b0c229476eef782f9f77b",
);
pub const MODEL_1_8B: (&str, &str) = (
    "Hy-MT2-1.8B-Q4_K_M.gguf",
    "dc5f44fcf1fa496ee7ad725982c0c8c553a4de00259b53af84c4b89fb0c06699",
);

/// Archivos del programa de traducción con sus huellas.
pub fn program_files() -> Vec<(&'static str, &'static str)> {
    SERVER_FILES
        .iter()
        .copied()
        .chain(
            LLAMA_FILES
                .iter()
                .copied()
                .filter(|(name, _)| *name != "llama-mtmd-cli.exe"),
        )
        .collect()
}

/// Idiomas de destino: (código, nombre para la instrucción del modelo, nombre en la interfaz).
pub const LANGUAGES: [(&str, &str, &str); 6] = [
    ("es", "Spanish", "Español"),
    ("en", "English", "Inglés"),
    ("fr", "French", "Francés"),
    ("de", "German", "Alemán"),
    ("it", "Italian", "Italiano"),
    ("pt", "Portuguese", "Portugués"),
];

/// Nombre del idioma para el modelo, a partir de su código.
pub fn language_name(code: &str) -> Option<&'static str> {
    LANGUAGES
        .iter()
        .find(|(c, _, _)| *c == code)
        .map(|(_, name, _)| *name)
}

pub struct Translator {
    pub program_dir: PathBuf,
    /// Archivo del modelo (uno de `MODEL_7B` o `MODEL_1_8B`).
    pub model: PathBuf,
    /// Servidor con el modelo cargado, arrancado con el primer bloque.
    server: Mutex<Option<Server>>,
}

/// Término del glosario: original y traducción obligada.
pub type Term = (String, String);

/// Aviso sobre un bloque traducido.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TranslationDoubt {
    /// El modelo no devolvió nada: se conserva el original.
    Empty,
    /// La traducción es mucho más larga o más corta que el original.
    Length,
    /// La salida es igual al original (no se tradujo).
    Untranslated,
    /// El marcado en línea no volvió intacto: se tradujo sin él.
    FormattingLost,
    /// Un término del glosario no aparece con su traducción obligada.
    Glossary,
    /// Faltan en la traducción cifras del original (suele indicar una frase omitida).
    Numbers,
    /// Falló el programa o se agotó el tiempo: se conserva el original.
    Failed,
}

impl TranslationDoubt {
    pub fn reason(&self) -> &'static str {
        match self {
            Self::Empty => "El traductor no devolvió nada; se conserva el original.",
            Self::Length => "La traducción es mucho más larga o más corta que el original.",
            Self::Untranslated => "El texto parece sin traducir.",
            Self::FormattingLost => "El formato (cursivas, negritas o enlaces) no se conservó.",
            Self::Glossary => "Un término del glosario no aparece con su traducción.",
            Self::Numbers => "Faltan cifras del original: puede faltar una frase.",
            Self::Failed => "El traductor falló en este bloque; se conserva el original.",
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TranslationReport {
    pub blocks: usize,
    pub translated: usize,
    pub reused: usize,
    /// (página, posición del bloque en la página, aviso).
    pub doubts: Vec<(u32, usize, TranslationDoubt)>,
    pub seconds: f64,
    /// EPUB ajeno: bloques con formato interno que quedan sin traducir.
    #[serde(default)]
    pub untouched: usize,
}

fn is_program(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("dll"))
}

/// Hilos para generar y para leer la instrucción: generar está limitado por la memoria y apenas
/// mejora con más de 8 (medido: 4, 8 y 16 hilos dan 10,3, 10,4 y 11,1 piezas por segundo).
fn threads() -> (usize, usize) {
    let available = std::thread::available_parallelism().map_or(4, |n| n.get());
    ((available / 2).clamp(1, 8), available.clamp(1, 16))
}

impl Translator {
    pub fn new(program_dir: PathBuf, model: PathBuf) -> Self {
        Self {
            program_dir,
            model,
            server: Mutex::new(None),
        }
    }

    /// Identidad del modelo para la reanudación (su huella, si es uno conocido).
    pub fn model_hash(&self) -> &'static str {
        let name = self
            .model
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("");
        [MODEL_7B, MODEL_1_8B]
            .iter()
            .find(|(n, _)| *n == name)
            .map_or("", |(_, hash)| *hash)
    }

    /// ¿Es el modelo de 1,8B? Usa otra plantilla de conversación que el de 7B (la de su GGUF).
    fn small(&self) -> bool {
        self.model_hash() == MODEL_1_8B.1
    }

    /// Huellas del programa y del modelo, y carpeta sin programas ajenos.
    pub fn verify(&self) -> Result<(), ReaderError> {
        let files = program_files();
        for (name, hash) in &files {
            if sha256_file(&self.program_dir.join(name)).ok().as_deref() != Some(*hash) {
                return Err(ReaderError::Unverified((*name).to_string()));
            }
        }
        for entry in fs::read_dir(&self.program_dir)
            .map_err(|_| ReaderError::Io)?
            .flatten()
        {
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_program(&entry.path())
                && !files.iter().any(|(n, _)| n.eq_ignore_ascii_case(&name))
            {
                return Err(ReaderError::Unverified(name));
            }
        }
        let expected = self.model_hash();
        let name = self
            .model
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        if expected.is_empty() || sha256_file(&self.model).ok().as_deref() != Some(expected) {
            return Err(ReaderError::Unverified(name));
        }
        Ok(())
    }

    /// Traduce un texto corto (un bloque o una celda) con los términos del glosario que contiene.
    /// El modelo se carga una sola vez y se mantiene en el servidor hasta que se suelta el
    /// traductor; cada bloque es una petición aparte, sin historial y con la misma semilla.
    pub fn translate(
        &self,
        text: &str,
        language: &str,
        terms: &[Term],
        work: &Path,
        cancel: &AtomicBool,
    ) -> Result<String, ReaderError> {
        let words = text.split_whitespace().count();
        // Margen amplio para la salida y un contexto que quepa entera con la instrucción (unas
        // tres letras por pieza).
        let tokens = (words * 4).clamp(64, 6144);
        let needed = (text.len() / 3 + 400 + tokens)
            .next_multiple_of(1024)
            .clamp(SERVER_CONTEXT, MAX_CONTEXT);
        let mut slot = self.server.lock().unwrap_or_else(|e| e.into_inner());
        // Un bloque excepcionalmente largo vuelve a arrancar el servidor con más contexto.
        if slot
            .as_mut()
            .is_some_and(|s| s.context < needed || !s.running())
        {
            drop(slot.take());
        }
        if slot.is_none() {
            *slot = Some(Server::start(self, needed, work, cancel)?);
        }
        let Some(server) = slot.as_ref() else {
            return Err(ReaderError::Failed);
        };
        let body = serde_json::json!({
            "prompt": prompt(text, language, terms, self.small()),
            "n_predict": tokens,
            "temperature": 0.7,
            "top_p": 0.6,
            "top_k": 20,
            "repeat_penalty": 1.05,
            "seed": 42,
            "cache_prompt": false,
        })
        .to_string();
        // Tiempo máximo generoso: unos 2 s por palabra, con un mínimo de 2 minutos.
        let timeout = Duration::from_secs((words as u64 * 2).max(120));
        let (port, key) = (server.port, server.key.clone());
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = sender.send(complete(port, &key, &body, timeout));
        });
        let started = Instant::now();
        let reply = loop {
            if cancel.load(Ordering::Relaxed) {
                drop(slot.take());
                return Err(ReaderError::Cancelled);
            }
            if started.elapsed() > timeout {
                drop(slot.take());
                return Err(ReaderError::TimedOut);
            }
            match receiver.recv_timeout(Duration::from_millis(50)) {
                Ok(reply) => break reply,
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                Err(mpsc::RecvTimeoutError::Disconnected) => return Err(ReaderError::Failed),
            }
        };
        match reply {
            Some(content) => Ok(clean_output(&content)),
            None => {
                // El servidor no respondió bien: se cierra y el bloque siguiente lo arranca de nuevo.
                drop(slot.take());
                Err(ReaderError::Failed)
            }
        }
    }
}

impl Drop for Translator {
    fn drop(&mut self) {
        let slot = self.server.get_mut().unwrap_or_else(|e| e.into_inner());
        drop(slot.take());
    }
}

/// Contexto con el que arranca el servidor: cabe de sobra un bloque normal y, con el 7B, ocupa
/// en torno a 1 GB, compatible con un equipo de 16 GB.
const SERVER_CONTEXT: usize = 8192;
const MAX_CONTEXT: usize = 16384;

/// `llama-server` con el modelo cargado. Escucha solo en 127.0.0.1, en un puerto libre elegido al
/// arrancar y con una clave nueva; se cierra al soltarlo (y con la app, por el trabajo de Windows).
struct Server {
    child: Child,
    port: u16,
    key: String,
    context: usize,
}

impl Server {
    fn start(
        translator: &Translator,
        context: usize,
        work: &Path,
        cancel: &AtomicBool,
    ) -> Result<Self, ReaderError> {
        fs::create_dir_all(work).map_err(|_| ReaderError::Io)?;
        // Dos intentos por si otro programa ocupa el puerto entre elegirlo y usarlo.
        let mut last = ReaderError::NotStarted;
        for _ in 0..2 {
            match Self::try_start(translator, context, work, cancel) {
                Ok(server) => return Ok(server),
                Err(ReaderError::Cancelled) => return Err(ReaderError::Cancelled),
                Err(_) => last = ReaderError::NotStarted,
            }
        }
        Err(last)
    }

    fn try_start(
        translator: &Translator,
        context: usize,
        work: &Path,
        cancel: &AtomicBool,
    ) -> Result<Self, ReaderError> {
        let port = TcpListener::bind(("127.0.0.1", 0))
            .and_then(|l| l.local_addr())
            .map_err(|_| ReaderError::Failed)?
            .port();
        let key = random_key();
        // La clave va en un archivo (no en la línea de órdenes) y se borra en cuanto se ha leído.
        let key_file = work.join("clave.txt");
        fs::write(&key_file, &key).map_err(|_| ReaderError::Io)?;
        let (threads, batch_threads) = threads();
        let mut command = Command::new(translator.program_dir.join(SERVER_FILES[0].0));
        let system = std::env::var("SYSTEMROOT").unwrap_or_else(|_| r"C:\Windows".into());
        command
            .args([
                "-m",
                &translator.model.to_string_lossy(),
                "-c",
                &context.to_string(),
                "-np",
                "1",
                "-t",
                &threads.to_string(),
                "-tb",
                &batch_threads.to_string(),
                "--host",
                "127.0.0.1",
                "--port",
                &port.to_string(),
                "--api-key-file",
                &key_file.to_string_lossy(),
                "--no-webui",
                "--no-webui-mcp-proxy",
                "--cache-ram",
                "0",
            ])
            .env_clear()
            .envs(std::env::vars().filter(|(k, _)| {
                matches!(
                    k.to_uppercase().as_str(),
                    "SYSTEMROOT" | "WINDIR" | "TEMP" | "TMP" | "NUMBER_OF_PROCESSORS"
                )
            }))
            .env("PATH", Path::new(&system).join("System32"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let child = match command.spawn() {
            Ok(child) => child,
            Err(_) => {
                let _ = fs::remove_file(&key_file);
                return Err(ReaderError::Failed);
            }
        };
        #[cfg(windows)]
        crate::child_process::tie_to_this_process(&child);
        let mut server = Self {
            child,
            port,
            key,
            context,
        };
        let ready = server.wait_ready(cancel);
        let _ = fs::remove_file(&key_file);
        ready.map(|()| server)
    }

    /// Espera a que el modelo esté cargado (hasta 5 minutos en un disco lento).
    fn wait_ready(&mut self, cancel: &AtomicBool) -> Result<(), ReaderError> {
        let started = Instant::now();
        let agent = agent(Duration::from_secs(2));
        let url = format!("http://127.0.0.1:{}/health", self.port);
        loop {
            if cancel.load(Ordering::Relaxed) {
                return Err(ReaderError::Cancelled);
            }
            if !self.running() {
                return Err(ReaderError::Failed);
            }
            if started.elapsed() > Duration::from_secs(300) {
                return Err(ReaderError::TimedOut);
            }
            if agent.get(&url).call().is_ok() {
                return Ok(());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    fn running(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .build()
        .into()
}

/// Pide una traducción al servidor; `None` si falla la conexión o la respuesta no es válida.
fn complete(port: u16, key: &str, body: &str, timeout: Duration) -> Option<String> {
    let mut response = agent(timeout)
        .post(&format!("http://127.0.0.1:{port}/completion"))
        .header("Authorization", &format!("Bearer {key}"))
        .content_type("application/json")
        .send(body)
        .ok()?;
    let text = response.body_mut().read_to_string().ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value.get("content")?.as_str().map(str::to_string)
}

/// Clave de un solo uso: semillas aleatorias del sistema, la hora y el proceso, resumidas en
/// SHA-256.
fn random_key() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = Sha256::new();
    for _ in 0..4 {
        let mut seed = std::collections::hash_map::RandomState::new().build_hasher();
        seed.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos()),
        );
        hasher.update(seed.finish().to_le_bytes());
    }
    hasher.update(std::process::id().to_le_bytes());
    format!("{:x}", hasher.finalize())
}

/// Instrucción para Hy-MT2: la plantilla normal o, con glosario, la de terminología de la ficha
/// del modelo, envuelta en la plantilla de conversación de cada tamaño (tomada de su GGUF: el 7B
/// usa `<|startoftext|>…<|extra_0|>` y el 1,8B, los marcadores `hy_`).
fn prompt(text: &str, language: &str, terms: &[Term], small: bool) -> String {
    let body = if terms.is_empty() {
        format!(
            "Translate the following text into {language}. Note that you should only output the \
             translated result without any additional explanation:\n{text}"
        )
    } else {
        let references: String = terms
            .iter()
            .map(|(source, target)| format!("{source} translates to {target}\n"))
            .collect();
        format!(
            "Reference the following translations:\n{references}\nTranslate the following text \
             into {language}. Note that you must ONLY output the translated result without any \
             additional explanation:\n{text}"
        )
    };
    if small {
        format!(
            "<\u{ff5c}hy_begin\u{2581}of\u{2581}sentence\u{ff5c}><\u{ff5c}hy_User\u{ff5c}>{body}<\u{ff5c}hy_Assistant\u{ff5c}>"
        )
    } else {
        format!("<|startoftext|>{body}<|extra_0|>")
    }
}

/// Quita los códigos de color, la marca de fin y los espacios sobrantes de la salida.
fn clean_output(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            // Secuencia ANSI: ESC [ ... letra.
            if chars.peek() == Some(&'[') {
                chars.next();
                for d in chars.by_ref() {
                    if d.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out.replace("[end of text]", "").trim().to_string()
}

/// Marcado en línea de un texto (para comprobar que vuelve intacto).
fn inline_marks(text: &str) -> Vec<&'static str> {
    let mut marks = Vec::new();
    for mark in ["**", "__", "`", "](", "[", "]"] {
        for _ in 0..text.matches(mark).count() {
            marks.push(mark);
        }
    }
    marks.push(if text.matches('*').count().is_multiple_of(2) {
        "*par"
    } else {
        "*impar"
    });
    marks
}

/// Destinos de los enlaces de un texto, que deben volver idénticos.
fn link_targets(text: &str) -> Vec<&str> {
    text.match_indices("](")
        .filter_map(|(i, _)| {
            let rest = &text[i + 2..];
            rest.find(')').map(|end| &rest[..end])
        })
        .collect()
}

/// Texto sin marcado en línea (los enlaces se quedan con su texto).
fn strip_inline(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(start) = rest.find('[') {
        let Some(mid) = rest[start..].find("](").map(|m| start + m) else {
            break;
        };
        let Some(end) = rest[mid..].find(')').map(|e| mid + e) else {
            break;
        };
        out.push_str(&rest[..start]);
        out.push_str(&rest[start + 1..mid]);
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out.replace("**", "")
        .replace("__", "")
        .replace(['*', '`'], "")
}

/// Comprueba una traducción frente a su original.
fn judge(source: &str, translated: &str) -> Option<TranslationDoubt> {
    let (a, b) = (source.chars().count(), translated.chars().count());
    if b == 0 {
        return Some(TranslationDoubt::Empty);
    }
    if source.split_whitespace().count() >= 5 && source.trim() == translated.trim() {
        return Some(TranslationDoubt::Untranslated);
    }
    if a >= 20 && (b * 10 < a * 3 || b > a * 3) {
        return Some(TranslationDoubt::Length);
    }
    None
}

/// Grupos de cifras de un texto (los separadores de millares y decimales cambian de un idioma a
/// otro, así que «1,000» y «1.000» dan los mismos grupos).
fn digit_groups(text: &str) -> Vec<&str> {
    let mut groups: Vec<&str> = text
        .split(|c: char| !c.is_ascii_digit())
        .filter(|g| !g.is_empty())
        .collect();
    groups.sort_unstable();
    groups
}

/// Aviso si alguna cifra del original no está en la traducción.
fn lost_numbers(source: &str, translated: &str) -> Option<TranslationDoubt> {
    let mut remaining = digit_groups(translated);
    for group in digit_groups(source) {
        match remaining.iter().position(|g| *g == group) {
            Some(at) => {
                remaining.remove(at);
            }
            None => return Some(TranslationDoubt::Numbers),
        }
    }
    None
}

/// Términos del glosario presentes en un texto (sin distinguir mayúsculas, palabra completa).
fn terms_in(text: &str, glossary: &[Term]) -> Vec<Term> {
    let lower = text.to_lowercase();
    glossary
        .iter()
        .filter(|(source, _)| contains_word(&lower, &source.to_lowercase()))
        .cloned()
        .collect()
}

fn contains_word(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return false;
    }
    haystack.match_indices(needle).any(|(i, _)| {
        let before = haystack[..i].chars().next_back();
        let after = haystack[i + needle.len()..].chars().next();
        !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric)
    })
}

/// Parte un bloque en un prefijo estructural («## », «- », «1. », «> ») y su texto.
fn split_prefix(line: &str) -> (&str, &str) {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    let mut end = indent;
    let bytes = trimmed.as_bytes();
    let hashes = bytes.iter().take_while(|b| **b == b'#').count();
    if (1..=6).contains(&hashes) && bytes.get(hashes) == Some(&b' ') {
        end += hashes + 1;
    } else if trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("> ") {
        end += 2;
    } else {
        let digits = bytes.iter().take_while(|b| b.is_ascii_digit()).count();
        if digits > 0 && trimmed[digits..].starts_with(". ") {
            end += digits + 2;
        }
    }
    (&line[..end], &line[end..])
}

/// Resultado de traducir un texto: la traducción y su aviso, si lo hay.
type Outcome = (String, Option<TranslationDoubt>);

/// Llamada al modelo: texto y términos del glosario que contiene.
pub type Run<'a> = dyn FnMut(&str, &[Term]) -> Result<String, ReaderError> + 'a;

/// Traduce un texto con sus guardas.
fn translate_text(text: &str, glossary: &[Term], run: &mut Run) -> Result<Outcome, ReaderError> {
    if !text.chars().any(char::is_alphabetic) {
        return Ok((text.to_string(), None));
    }
    let terms = terms_in(text, glossary);
    let translated = match run(text, &terms) {
        Ok(t) => t,
        Err(error @ (ReaderError::Cancelled | ReaderError::NotStarted)) => return Err(error),
        Err(_) => return Ok((text.to_string(), Some(TranslationDoubt::Failed))),
    };
    let (translated, mut doubt) = if inline_marks(text) != inline_marks(&translated)
        || link_targets(text) != link_targets(&translated)
    {
        // El marcado no volvió intacto: se traduce el texto sin él.
        let plain = strip_inline(text);
        let again = match run(&plain, &terms) {
            Ok(t) => t,
            Err(error @ (ReaderError::Cancelled | ReaderError::NotStarted)) => return Err(error),
            Err(_) => return Ok((text.to_string(), Some(TranslationDoubt::Failed))),
        };
        if judge(&plain, &again) == Some(TranslationDoubt::Empty) {
            return Ok((text.to_string(), Some(TranslationDoubt::Empty)));
        }
        (again, Some(TranslationDoubt::FormattingLost))
    } else {
        match judge(text, &translated) {
            Some(TranslationDoubt::Empty) => {
                return Ok((text.to_string(), Some(TranslationDoubt::Empty)));
            }
            doubt => (translated, doubt),
        }
    };
    let lower = translated.to_lowercase();
    if doubt.is_none()
        && terms
            .iter()
            .any(|(_, target)| !lower.contains(&target.to_lowercase()))
    {
        doubt = Some(TranslationDoubt::Glossary);
    }
    Ok((translated, doubt))
}

/// Traduce un bloque de Markdown conservando su estructura.
fn translate_block(block: &str, glossary: &[Term], run: &mut Run) -> Result<Outcome, ReaderError> {
    let lines: Vec<&str> = block.lines().collect();
    // Tabla: celda a celda, sin tocar la fila separadora.
    if lines.iter().all(|l| l.trim_start().starts_with('|')) {
        let mut doubt = None;
        let mut out = Vec::new();
        for line in lines {
            let separator = line.chars().all(|c| matches!(c, '|' | '-' | ':' | ' '));
            if separator {
                out.push(line.to_string());
                continue;
            }
            let cells: Vec<&str> = line.trim().trim_matches('|').split('|').collect();
            let mut translated = Vec::new();
            for cell in cells {
                let (text, d) = translate_text(cell.trim(), glossary, run)?;
                doubt = doubt.or(d);
                translated.push(text.replace('|', "\\|").replace('\n', " "));
            }
            out.push(format!("| {} |", translated.join(" | ")));
        }
        return Ok((out.join("\n"), doubt));
    }
    // Listas y citas: línea a línea; un párrafo: entero, con su prefijo de título si lo tiene.
    let line_items = lines.len() > 1 && lines.iter().all(|l| !split_prefix(l).0.is_empty());
    if line_items {
        let mut doubt = None;
        let mut out = Vec::new();
        for line in lines {
            let (prefix, text) = split_prefix(line);
            let (translated, d) = translate_text(text, glossary, run)?;
            doubt = doubt.or(d);
            out.push(format!("{prefix}{}", translated.replace('\n', " ")));
        }
        return Ok((out.join("\n"), doubt));
    }
    let (prefix, text) = split_prefix(block);
    let joined = text.lines().map(str::trim).collect::<Vec<_>>().join(" ");
    let (translated, doubt) = translate_text(&joined, glossary, run)?;
    // Un párrafo no puede partirse en varios bloques ni convertirse en título.
    let translated = translated
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    Ok((format!("{prefix}{translated}"), doubt))
}

fn skip_block(block: &str) -> bool {
    let trimmed = block.trim();
    trimmed.is_empty()
        || trimmed.starts_with("<!--")
        || trimmed.starts_with("![")
        || trimmed.starts_with("```")
        || trimmed.starts_with("$$")
}

/// Bloques de un Markdown: párrafos separados por línea en blanco, salvo dentro de un bloque de
/// código, que va entero.
fn blocks(markdown: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    let mut current: Vec<&str> = Vec::new();
    let mut fenced = false;
    for line in markdown.split('\n') {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        }
        if line.trim().is_empty() && !fenced {
            if !current.is_empty() {
                out.push(current.join("\n"));
                current.clear();
            }
            continue;
        }
        current.push(line);
    }
    if !current.is_empty() {
        out.push(current.join("\n"));
    }
    out
}

/// Versión de las instrucciones al modelo: entra en la clave de los bloques guardados, de modo
/// que un cambio de plantilla nunca reutiliza traducciones hechas con la anterior.
const PROMPT_VERSION: &str = "hy-mt2-plantillas-2";

/// Archivo del bloque traducido: huella del texto, idioma, modelo, glosario y versión.
fn cache_name(language: &str, model_hash: &str, terms: &[Term], source: &str) -> String {
    let key = format!(
        "{:x}",
        Sha256::digest(
            format!("{PROMPT_VERSION}\n{language}\n{model_hash}\n{terms:?}\n{source}").as_bytes()
        )
    );
    format!("{}.json", &key[..32])
}

/// ¿Es un párrafo corriente (sin prefijo de título, lista o cita, ni tabla)?
fn plain_paragraph(block: &str) -> bool {
    let block = block.trim();
    !skip_block(block) && !block.starts_with('|') && split_prefix(block).0.is_empty()
}

/// Párrafos cortados por un salto de página: (índice del primer trozo, índice del segundo). El
/// primero no termina en puntuación final y el segundo empieza en minúscula.
fn continuations(blocks: &[String]) -> HashMap<usize, usize> {
    let mut pairs = HashMap::new();
    for (i, block) in blocks.iter().enumerate() {
        if !block.trim().starts_with("<!-- page:") || i == 0 {
            continue;
        }
        let Some(before) = (0..i)
            .rev()
            .find(|&j| is_content(&blocks[j]) || is_marker(&blocks[j]))
        else {
            continue;
        };
        let Some(after) = (i + 1..blocks.len()).find(|&j| is_content(&blocks[j])) else {
            continue;
        };
        if is_marker(&blocks[before])
            || blocks[i + 1..after].iter().any(|b| is_marker(b))
            || !plain_paragraph(&blocks[before])
            || !plain_paragraph(&blocks[after])
            || pairs.contains_key(&after)
        {
            continue;
        }
        let ends_open = blocks[before]
            .trim_end()
            .chars()
            .next_back()
            .is_some_and(|c| !".!?:;…\"”»)]'’*_".contains(c));
        let starts_lower = blocks[after]
            .chars()
            .find(|c| c.is_alphabetic())
            .is_some_and(char::is_lowercase);
        if ends_open && starts_lower {
            pairs.insert(before, after);
        }
    }
    pairs
}

fn is_marker(block: &str) -> bool {
    block.trim().starts_with("<!-- page:")
}

/// Bloque de contenido, contado igual que en la revisión (sin imágenes ni comentarios).
fn is_content(block: &str) -> bool {
    let block = block.trim();
    !block.is_empty() && !block.starts_with("![") && !block.starts_with("<!--")
}

/// Reparte una traducción entre dos trozos en proporción a las palabras del original.
fn split_translation(
    translated: &str,
    first_words: usize,
    second_words: usize,
) -> (String, String) {
    let words: Vec<&str> = translated.split_whitespace().collect();
    if words.len() < 2 {
        return (translated.to_string(), String::new());
    }
    let cut = (words.len() * first_words)
        .div_ceil((first_words + second_words).max(1))
        .clamp(1, words.len() - 1);
    (words[..cut].join(" "), words[cut..].join(" "))
}

/// Traduce un Markdown con marcas de página. `cache` guarda cada bloque traducido (clave: huella
/// del bloque, idioma, modelo y glosario) para reanudar sin repetir trabajo. Un párrafo cortado
/// por un salto de página se traduce entero y la traducción se reparte entre sus dos trozos, en
/// proporción a sus palabras, para que cada página conserve su parte.
pub fn translate_markdown(
    markdown: &str,
    language: &str,
    model_hash: &str,
    glossary: &[Term],
    cache: &Path,
    run: &mut Run,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<(String, TranslationReport), ReaderError> {
    let started = Instant::now();
    fs::create_dir_all(cache).map_err(|_| ReaderError::Io)?;
    let blocks = blocks(&markdown.replace("\r\n", "\n"));
    let joined = continuations(&blocks);
    let total = blocks.iter().filter(|b| !skip_block(b)).count();
    let mut report = TranslationReport {
        blocks: total,
        ..Default::default()
    };
    let mut out = Vec::with_capacity(blocks.len());
    let mut second_parts: HashMap<usize, String> = HashMap::new();
    let (mut page, mut index, mut done) = (0u32, 0usize, 0usize);
    for (position, block) in blocks.iter().enumerate() {
        if let Some(rest) = block.trim().strip_prefix("<!-- page:") {
            page = rest
                .trim_end_matches("-->")
                .trim()
                .parse()
                .unwrap_or(page + 1);
            index = 0;
            out.push(block.clone());
            continue;
        }
        if skip_block(block) {
            if is_content(block) {
                index += 1;
            }
            out.push(block.clone());
            continue;
        }
        if let Some(second) = second_parts.remove(&position) {
            out.push(second);
            index += 1;
            done += 1;
            progress(done, total);
            continue;
        }
        let partner = joined.get(&position).copied();
        let source = match partner {
            Some(after) => format!("{} {}", block.trim(), blocks[after].trim()),
            None => block.trim().to_string(),
        };
        let terms = terms_in(&source, glossary);
        let cached = cache.join(cache_name(language, model_hash, &terms, &source));
        let record: Option<Outcome> = fs::read(&cached)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        let (translated, doubt) = match record {
            Some(record) => {
                report.reused += 1;
                record
            }
            None => {
                let result = translate_block(&source, &terms, run)?;
                let temporary = cached.with_extension("tmp");
                if fs::write(&temporary, serde_json::to_vec(&result).unwrap_or_default()).is_ok() {
                    let _ = fs::rename(&temporary, &cached);
                }
                report.translated += 1;
                result
            }
        };
        // La comprobación de cifras se hace siempre, también sobre lo reutilizado.
        let doubt = doubt.or_else(|| lost_numbers(&source, &translated));
        if let Some(doubt) = doubt {
            report.doubts.push((page, index, doubt));
        }
        let translated = match partner {
            Some(after) => {
                let (first, second) = split_translation(
                    &translated,
                    block.split_whitespace().count(),
                    blocks[after].split_whitespace().count(),
                );
                second_parts.insert(after, second);
                first
            }
            None => translated,
        };
        out.push(translated);
        index += 1;
        done += 1;
        progress(done, total);
    }
    report.seconds = started.elapsed().as_secs_f64();
    Ok((out.join("\n\n") + "\n", report))
}

/// Traduce un EPUB ajeno en su sitio: el texto de cada bloque sin formato interno se sustituye por
/// su traducción y el paquete declara el nuevo idioma; todo lo demás (estilos, imágenes, índice,
/// bloques con formato interno) queda como estaba. En el informe, la «página» es el capítulo
/// (contado desde 1) y la posición, el bloque.
#[allow(clippy::too_many_arguments)]
pub fn translate_epub<R: Read + Seek>(
    package: &mut EpubPackage<R>,
    language: &str,
    code: &str,
    model_hash: &str,
    glossary: &[Term],
    cache: &Path,
    run: &mut Run,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<TranslationReport, ReaderError> {
    let started = Instant::now();
    fs::create_dir_all(cache).map_err(|_| ReaderError::Io)?;
    let chapters: Vec<_> = (0..package.chapters.len())
        .map(|chapter| package.blocks(chapter))
        .collect();
    let has_letters = |text: &str| text.chars().any(char::is_alphabetic);
    let total = chapters
        .iter()
        .flatten()
        .filter(|b| b.editable && has_letters(&b.text))
        .count();
    let mut report = TranslationReport {
        blocks: total,
        untouched: chapters
            .iter()
            .flatten()
            .filter(|b| !b.editable && has_letters(&b.text))
            .count(),
        ..Default::default()
    };
    let mut done = 0;
    for block in chapters.iter().flatten() {
        if !block.editable || !has_letters(&block.text) {
            continue;
        }
        let source = block.text.split_whitespace().collect::<Vec<_>>().join(" ");
        let terms = terms_in(&source, glossary);
        let cached = cache.join(cache_name(language, model_hash, &terms, &source));
        let record: Option<Outcome> = fs::read(&cached)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok());
        let (translated, doubt) = match record {
            Some(record) => {
                report.reused += 1;
                record
            }
            None => {
                let result = translate_text(&source, &terms, run)?;
                let temporary = cached.with_extension("tmp");
                if fs::write(&temporary, serde_json::to_vec(&result).unwrap_or_default()).is_ok() {
                    let _ = fs::rename(&temporary, &cached);
                }
                report.translated += 1;
                result
            }
        };
        let doubt = doubt.or_else(|| lost_numbers(&source, &translated));
        if let Some(doubt) = doubt {
            report
                .doubts
                .push((block.chapter as u32 + 1, block.index, doubt));
        }
        package
            .set_text(block.chapter, block.index, &translated.replace('\n', " "))
            .map_err(|_| ReaderError::Failed)?;
        done += 1;
        progress(done, total);
    }
    package
        .set_language(code)
        .map_err(|_| ReaderError::Failed)?;
    report.seconds = started.elapsed().as_secs_f64();
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// «Traductor» de prueba: pone en mayúsculas y cuenta las llamadas.
    fn upper(calls: &mut usize) -> impl FnMut(&str, &[Term]) -> Result<String, ReaderError> + '_ {
        move |text: &str, _: &[Term]| {
            *calls += 1;
            Ok(text.to_uppercase())
        }
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("liblevo-trad-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn structure_stays_outside_the_model() {
        let markdown = "<!-- page: 1 -->\n\n## Un título\n\nUn párrafo\npartido en dos líneas.\n\n\
            ![](fig.png)\n\n- uno\n- dos\n\n| A | b |\n| --- | --- |\n| c | 12 |\n\n```\ncódigo\n\n\
            más código\n```";
        let dir = temp("estructura");
        let mut calls = 0;
        let (out, report) = translate_markdown(
            markdown,
            "Spanish",
            "m",
            &[],
            &dir,
            &mut upper(&mut calls),
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(
            out,
            "<!-- page: 1 -->\n\n## UN TÍTULO\n\nUN PÁRRAFO PARTIDO EN DOS LÍNEAS.\n\n![](fig.png)\n\n\
             - UNO\n- DOS\n\n| A | B |\n| --- | --- |\n| C | 12 |\n\n```\ncódigo\n\nmás código\n```\n"
        );
        assert_eq!(report.blocks, 4);
        assert!(report.doubts.is_empty());
        // Reanudar: nada se vuelve a traducir; otro modelo, sí.
        let before = calls;
        let (again, report) = translate_markdown(
            markdown,
            "Spanish",
            "m",
            &[],
            &dir,
            &mut upper(&mut calls),
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!((again, calls, report.reused), (out, before, 4));
        let (_, report) = translate_markdown(
            markdown,
            "Spanish",
            "otro",
            &[],
            &dir,
            &mut upper(&mut calls),
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(report.reused, 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_paragraph_cut_by_a_page_break_is_translated_whole() {
        let markdown = "<!-- page: 1 -->\n\nEl reloj de su padre y\n\n![](f.png)\n\n\
            <!-- page: 2 -->\n\nde su abuelo brillaba.\n\nOtro párrafo.\n\n<!-- page: 3 -->\n\n\
            Termina aquí.\n\n<!-- page: 4 -->\n\nnueva frase en minúscula.";
        let dir = temp("continuacion");
        let mut seen = Vec::new();
        let mut record = |text: &str, _: &[Term]| {
            seen.push(text.to_string());
            Ok(text.to_uppercase())
        };
        let (out, report) = translate_markdown(
            markdown,
            "Spanish",
            "m",
            &[],
            &dir,
            &mut record,
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!(
            seen,
            vec![
                "El reloj de su padre y de su abuelo brillaba.",
                "Otro párrafo.",
                "Termina aquí.",
                "nueva frase en minúscula."
            ]
        );
        assert_eq!(
            out,
            "<!-- page: 1 -->\n\nEL RELOJ DE SU PADRE Y\n\n![](f.png)\n\n<!-- page: 2 -->\n\n\
             DE SU ABUELO BRILLABA.\n\nOTRO PÁRRAFO.\n\n<!-- page: 3 -->\n\nTERMINA AQUÍ.\n\n\
             <!-- page: 4 -->\n\nNUEVA FRASE EN MINÚSCULA.\n"
        );
        assert_eq!((report.blocks, report.translated), (5, 4));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_foreign_epub_is_translated_in_place() {
        use std::io::{Cursor, Write};
        use zip::write::SimpleFileOptions;
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = SimpleFileOptions::default();
        zip.start_file("mimetype", options).unwrap();
        zip.write_all(b"application/epub+zip").unwrap();
        zip.start_file("META-INF/container.xml", options).unwrap();
        zip.write_all(
            br#"<container><rootfiles><rootfile full-path="c.opf"/></rootfiles></container>"#,
        )
        .unwrap();
        zip.start_file("c.opf", options).unwrap();
        zip.write_all(
            br#"<package><metadata><dc:language>en</dc:language></metadata><manifest><item id="a" href="a.xhtml"/></manifest><spine><itemref idref="a"/></spine></package>"#,
        )
        .unwrap();
        zip.start_file("a.xhtml", options).unwrap();
        zip.write_all(
            "<html lang=\"en\"><body><h1>Title</h1><p>Some text.</p><p>With <em>style</em>.</p><p>42</p></body></html>"
                .as_bytes(),
        )
        .unwrap();
        let bytes = zip.finish().unwrap().into_inner();
        let mut package = EpubPackage::open(Cursor::new(bytes)).unwrap();
        let dir = temp("epub");
        let mut calls = 0;
        let report = translate_epub(
            &mut package,
            "Spanish",
            "es",
            "m",
            &[],
            &dir,
            &mut upper(&mut calls),
            &mut |_, _| {},
        )
        .unwrap();
        assert_eq!((report.blocks, report.untouched, calls), (2, 1, 2));
        let texts: Vec<String> = package.blocks(0).into_iter().map(|b| b.text).collect();
        assert_eq!(texts, vec!["TITLE", "SOME TEXT.", "With style.", "42"]);
        let saved = package.save(Cursor::new(Vec::new())).unwrap().into_inner();
        let mut archive = zip::ZipArchive::new(Cursor::new(saved)).unwrap();
        let mut opf = String::new();
        archive
            .by_name("c.opf")
            .unwrap()
            .read_to_string(&mut opf)
            .unwrap();
        assert!(opf.contains("<dc:language>es</dc:language>"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn guards_keep_the_original_and_flag_doubts() {
        let mut empty = |_: &str, _: &[Term]| Ok(String::new());
        assert_eq!(
            translate_text("Hola, ¿qué tal?", &[], &mut empty).unwrap(),
            ("Hola, ¿qué tal?".to_string(), Some(TranslationDoubt::Empty))
        );
        let mut same = |t: &str, _: &[Term]| Ok(t.to_string());
        assert_eq!(
            translate_text("This sentence has more than five words.", &[], &mut same)
                .unwrap()
                .1,
            Some(TranslationDoubt::Untranslated)
        );
        let mut long = |t: &str, _: &[Term]| Ok(t.repeat(5));
        assert_eq!(
            translate_text("A short but real sentence here.", &[], &mut long)
                .unwrap()
                .1,
            Some(TranslationDoubt::Length)
        );
        // Un énfasis que no vuelve: se traduce sin marcado y se avisa.
        let mut drop_marks = |t: &str, _: &[Term]| Ok(t.replace("**", "").to_uppercase());
        assert_eq!(
            translate_text(
                "Un **gran** día con [enlace](http://x).",
                &[],
                &mut drop_marks
            )
            .unwrap(),
            (
                "UN GRAN DÍA CON ENLACE.".to_string(),
                Some(TranslationDoubt::FormattingLost)
            )
        );
        // Un enlace cuyo destino cambia también cuenta como marcado perdido.
        let mut change_link = |t: &str, _: &[Term]| Ok(t.replace("http://x", "http://y"));
        assert_eq!(
            translate_text("Mira [aquí](http://x) ahora.", &[], &mut change_link)
                .unwrap()
                .1,
            Some(TranslationDoubt::FormattingLost)
        );
        // Sin letras (números, símbolos): no se traduce.
        let mut never =
            |_: &str, _: &[Term]| -> Result<String, ReaderError> { panic!("no debe llamarse") };
        assert_eq!(
            translate_text("12 345", &[], &mut never).unwrap().0,
            "12 345"
        );
    }

    #[test]
    fn the_glossary_is_passed_and_checked() {
        let glossary = vec![
            ("printing press".to_string(), "imprenta".to_string()),
            ("press".to_string(), "prensa".to_string()),
            ("ink".to_string(), "tinta".to_string()),
        ];
        // Solo los términos presentes (palabra completa: «ink» no está en «thinking»).
        assert_eq!(
            terms_in("The Printing Press needs thinking.", &glossary),
            glossary[..2].to_vec()
        );
        let mut seen = Vec::new();
        let mut obeys = |_: &str, terms: &[Term]| {
            seen = terms.to_vec();
            Ok("La imprenta y la prensa cambiaron todo.".to_string())
        };
        let outcome = translate_text(
            "The printing press changed everything.",
            &glossary,
            &mut obeys,
        )
        .unwrap();
        assert_eq!((outcome.1, seen.len()), (None, 2));
        let mut ignores = |_: &str, _: &[Term]| Ok("La máquina cambió todo.".to_string());
        assert_eq!(
            translate_text("The press changed everything.", &glossary, &mut ignores)
                .unwrap()
                .1,
            Some(TranslationDoubt::Glossary)
        );
        assert!(
            prompt("x", "Spanish", &glossary[2..], false).contains("ink translates to tinta\n")
        );
        assert!(
            prompt("x", "Spanish", &[], true)
                .starts_with("<｜hy_begin▁of▁sentence｜><｜hy_User｜>")
        );
        assert!(prompt("x", "Spanish", &[], true).ends_with("x<｜hy_Assistant｜>"));
    }

    #[test]
    fn missing_figures_are_flagged() {
        assert_eq!(
            lost_numbers("Pages 4171–4186, 2019.", "Páginas 4171-4186 (2019)."),
            None
        );
        assert_eq!(
            lost_numbers("It cost $1,000.50.", "Costó 1.000,50 $."),
            None
        );
        assert_eq!(
            lost_numbers("In 2019, pages 4171–4186.", "En las páginas 4171."),
            Some(TranslationDoubt::Numbers)
        );
        // Una cifra repetida debe aparecer las mismas veces.
        assert_eq!(
            lost_numbers("7 and 7", "7"),
            Some(TranslationDoubt::Numbers)
        );
    }

    #[test]
    fn model_output_is_cleaned() {
        assert_eq!(
            clean_output("\u{1b}[33m\u{1b}[0mEl río. [end of text]\n\n"),
            "El río."
        );
        // Servidor y su biblioteca, más lo compartido con la relectura salvo su programa.
        assert_eq!(program_files().len(), 23);
        assert!(program_files().iter().any(|(n, _)| *n == "mtmd.dll"));
        assert_eq!(language_name("es"), Some("Spanish"));
    }

    #[test]
    fn server_keys_are_new_each_time() {
        let (a, b) = (random_key(), random_key());
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }
}
