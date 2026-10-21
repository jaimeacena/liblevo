//! Relectura de páginas dudosas de un escaneo con el modelo visual OvisOCR2: programa oficial
//! `llama-mtmd-cli` de llama.cpp b10566 (MIT) en un proceso
//! aparte y modelo en GGUF (Apache-2.0), todo comprobado por huella SHA-256 antes de usarlo.
//!
//! La relectura solo sustituye al OCR si pasa la guarda (`accept`): conserva al menos el 75 % de las
//! palabras del OCR y no es más de 1,5 veces más larga. Así un corte o un bucle del modelo nunca
//! pierde texto ni lo inventa en masa; la página conserva su OCR y sigue marcada como dudosa.

use crate::doubt::markdown_pages;
use crate::html_tables::tables_to_markdown;
use crate::xberg_reader::{ReaderError, sha256_file};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Archivos del programa oficial de llama.cpp b10566 (Windows x64, CPU) que necesita
/// `llama-mtmd-cli`. La carpeta no puede contener otros programas ni bibliotecas: llama.cpp carga
/// por su cuenta las bibliotecas `ggml-*` que encuentra junto a él.
pub const PROGRAM_FILES: [(&str, &str); 22] = [
    (
        "llama-mtmd-cli.exe",
        "5c9c756f3485c88227d7c323cfa8c7767064baeba2c7ee811967e1ce67bace1c",
    ),
    (
        "ggml-base.dll",
        "fb5667744745667362718680b0f9676652715c71d879a38a3806e6053379bfaa",
    ),
    (
        "ggml-cpu-alderlake.dll",
        "b6e6926136c576be6799259a4cfa24c197ac9103b10c4e5e79093e0241312b46",
    ),
    (
        "ggml-cpu-cannonlake.dll",
        "727d8e78dbef74d1d982b7ad05cfa02476e74b21c1dbec99c8e6572638ed5dfe",
    ),
    (
        "ggml-cpu-cascadelake.dll",
        "a55f32797ee4f578781ce964d5ca0f7e85704fcd5ae738d55cbbe56adb88f909",
    ),
    (
        "ggml-cpu-cooperlake.dll",
        "7fdd34c2021b875a4b39be65747e4339250feab42a6f373743489522e142d994",
    ),
    (
        "ggml-cpu-haswell.dll",
        "851af4b2d5b10b0e078efcd1446a50497e42bee6e4cb9f53c9260515ccb4d83e",
    ),
    (
        "ggml-cpu-icelake.dll",
        "afb8146a777e2ab9cf1d84aca8ae0d58ecddb8375cc4acefbdcd3339238651ba",
    ),
    (
        "ggml-cpu-ivybridge.dll",
        "2ab20d141eb113a58b7450a4002afb8a478b29319e0526259b972e523ba0c791",
    ),
    (
        "ggml-cpu-piledriver.dll",
        "c36a261cd1a243e9c8655e889a0d1be111a7f828af567fdaaee5b88eac646016",
    ),
    (
        "ggml-cpu-sandybridge.dll",
        "43103b9a8c4ee066df78ff14c6ab4d45d01919f519b3e31e553c72aa67244362",
    ),
    (
        "ggml-cpu-sapphirerapids.dll",
        "5e78f73720ca8df99171d72857299297de0559ca54d7b5f1924d2a8f178923ab",
    ),
    (
        "ggml-cpu-skylakex.dll",
        "0b96c3e400ad7a23ad6cc7d3de6b15daef31fe6ec56dae346ad8406bf3684212",
    ),
    (
        "ggml-cpu-sse42.dll",
        "572dd1ba3be11514d8ccf47628e87fed084400071d0d28b1a878acc056d057ff",
    ),
    (
        "ggml-cpu-x64.dll",
        "e88ce8d5d7300b5c0980730178431269a057d604b9ca87784222847ec95db85e",
    ),
    (
        "ggml-cpu-zen4.dll",
        "76d986936a19b828785eeedaa2c5bb415a87a1d2f58fd9be5a03fc0a41b7ffcf",
    ),
    (
        "ggml-rpc.dll",
        "b07dc2ed45976884cc182e190be06c260297f030289ed95b34d33e5420dedf6f",
    ),
    (
        "ggml.dll",
        "25f26d757e39e6b9afbf61e68ec4479fd73e4c8dc0572fe927d937ba61ee3e62",
    ),
    (
        "libomp.dll",
        "a12116ba72d1d6820407cf30be23da04ce79d6bb8a71a5ee71759c5a1faa6f1c",
    ),
    (
        "llama-common.dll",
        "dcbba9aaabb23435fdffb3c35b3ebd8ddfd525e0ae052b76a9cdaf0caa905c60",
    ),
    (
        "llama.dll",
        "0db020c722bd869d9e410224bb4674a6b6e080cf9633b82d2dd92b661a79b693",
    ),
    (
        "mtmd.dll",
        "4977f7e0d002859da1db24505369741ca90aa8dac2f9916e51b31056bc803874",
    ),
];

/// Modelo OvisOCR2 en GGUF (F16) y su proyector visual.
pub const MODEL_FILES: [(&str, &str); 2] = [
    (
        "OvisOCR2-F16.gguf",
        "bd8f2c62c930d74590abcb5f65f79e898672e458d6044e8b2e885daea948592e",
    ),
    (
        "mmproj-F16.gguf",
        "7e371704e8f3c88638830e52866dfa88b2fd383155e3a0f9484e4ce037fdd42f",
    ),
];

/// Instrucción del modelo, la misma con la que se midió con el conjunto.
const PROMPT: &str = "Extract all readable content from the image in natural human reading order \
    and output the result as a single Markdown document. For charts or images, represent them using \
    an HTML image tag: <img src=\"images/bbox_{left}_{top}_{right}_{bottom}.jpg\" />, where left, \
    top, right, bottom are bounding box coordinates scaled to [0, 1000). Format formulas as LaTeX. \
    Format tables as HTML: <table>...</table>. Transcribe all other text as standard Markdown. \
    Preserve the original text without translation or paraphrasing.";

/// Parte mínima de las palabras del OCR que la relectura debe conservar.
const MIN_KEPT: f64 = 0.75;
/// Longitud máxima de la relectura frente al OCR (más larga suele ser un bucle del modelo).
const MAX_LENGTH_RATIO: f64 = 1.5;

pub struct VisualReader {
    /// Carpeta con `llama-mtmd-cli.exe` y sus bibliotecas.
    pub program_dir: PathBuf,
    /// Carpeta con los dos archivos GGUF del modelo.
    pub model_dir: PathBuf,
}

fn is_program(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("exe") || e.eq_ignore_ascii_case("dll"))
}

impl VisualReader {
    /// Comprueba las huellas del programa y del modelo, y que la carpeta del programa no tenga
    /// otros programas o bibliotecas. Lee 1,7 GB: se hace una vez por conversión.
    pub fn verify(&self) -> Result<(), ReaderError> {
        for (name, hash) in PROGRAM_FILES {
            if sha256_file(&self.program_dir.join(name)).ok().as_deref() != Some(hash) {
                return Err(ReaderError::Unverified(name.to_string()));
            }
        }
        let entries = fs::read_dir(&self.program_dir).map_err(|_| ReaderError::Io)?;
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if is_program(&path)
                && !PROGRAM_FILES
                    .iter()
                    .any(|(n, _)| n.eq_ignore_ascii_case(&name))
            {
                return Err(ReaderError::Unverified(name));
            }
        }
        for (name, hash) in MODEL_FILES {
            if sha256_file(&self.model_dir.join(name)).ok().as_deref() != Some(hash) {
                return Err(ReaderError::Unverified(name.to_string()));
            }
        }
        Ok(())
    }

    fn arguments(&self, image: &Path, max_tokens: u32) -> Vec<String> {
        let mut arguments: Vec<String> = vec![
            "-m".into(),
            self.model_dir
                .join(MODEL_FILES[0].0)
                .to_string_lossy()
                .into_owned(),
            "--mmproj".into(),
            self.model_dir
                .join(MODEL_FILES[1].0)
                .to_string_lossy()
                .into_owned(),
            "--image".into(),
            image.to_string_lossy().into_owned(),
            "-p".into(),
            PROMPT.into(),
        ];
        arguments.extend(
            [
                "-c",
                "12288",
                "--temp",
                "0",
                "--repeat-penalty",
                "1.1",
                "--repeat-last-n",
                "64",
                "-t",
                "4",
                "-fa",
                "on",
                "-n",
            ]
            .map(String::from),
        );
        arguments.push(max_tokens.to_string());
        arguments
    }

    /// Lee una página ya convertida en imagen. `work` es una carpeta privada del intento. Devuelve
    /// el texto del modelo sin limpiar.
    pub fn read_page(
        &self,
        image: &Path,
        work: &Path,
        max_tokens: u32,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> Result<String, ReaderError> {
        fs::create_dir_all(work).map_err(|_| ReaderError::Io)?;
        let output = work.join("visual.txt");
        let sink = fs::File::create(&output).map_err(|_| ReaderError::Io)?;
        let system = std::env::var("SYSTEMROOT").unwrap_or_else(|_| r"C:\Windows".into());
        let mut command = Command::new(self.program_dir.join("llama-mtmd-cli.exe"));
        command
            .args(self.arguments(image, max_tokens))
            .env_clear()
            .envs(std::env::vars().filter(|(k, _)| {
                matches!(
                    k.to_uppercase().as_str(),
                    "SYSTEMROOT" | "WINDIR" | "TEMP" | "TMP" | "NUMBER_OF_PROCESSORS"
                )
            }))
            .env("PATH", Path::new(&system).join("System32"))
            .env("OMP_NUM_THREADS", "4")
            .stdin(Stdio::null())
            .stdout(sink)
            .stderr(Stdio::null());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            command.creation_flags(CREATE_NO_WINDOW);
        }
        let mut child = command.spawn().map_err(|_| ReaderError::Failed)?;
        #[cfg(windows)]
        crate::child_process::tie_to_this_process(&child);
        let started = Instant::now();
        let status = loop {
            if cancel.load(Ordering::Relaxed) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ReaderError::Cancelled);
            }
            if started.elapsed() > timeout {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ReaderError::TimedOut);
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => std::thread::sleep(Duration::from_millis(100)),
                Err(_) => return Err(ReaderError::Failed),
            }
        };
        let text = fs::read(&output).map_err(|_| ReaderError::Failed)?;
        let _ = fs::remove_file(&output);
        if !status.success() {
            return Err(ReaderError::Failed);
        }
        Ok(String::from_utf8_lossy(&text).into_owned())
    }
}

/// Límite de salida del modelo según las palabras que leyó el OCR: deja margen para el marcado de
/// las tablas y corta pronto un bucle de repetición.
pub fn max_tokens_for(ocr_words: usize) -> u32 {
    (ocr_words.saturating_mul(6)).clamp(1024, 4096) as u32
}

/// Texto del modelo listo para el Markdown: sin razonamiento interno, sin marcas de imagen (en un
/// escaneo las figuras llegan por otro camino) y con las tablas HTML convertidas en tablas Markdown.
pub fn clean(raw: &str) -> String {
    let mut text = raw.to_string();
    while let Some(start) = text.find("<think>") {
        let end = text[start..]
            .find("</think>")
            .map_or(text.len(), |e| start + e + "</think>".len());
        text.replace_range(start..end, "");
    }
    // Cada figura queda como marca con su caja; una caja repetida (bucle del modelo) se quita.
    let mut seen: Vec<FigureBox> = Vec::new();
    while let Some(start) = text.find("<img") {
        let end = text[start..]
            .find('>')
            .map_or(text.len(), |e| start + e + 1);
        let replacement = match FigureBox::from_tag(&text[start..end]) {
            Some(figure) if !seen.contains(&figure) => {
                seen.push(figure);
                format!("\n\n{}\n\n", figure.marker())
            }
            _ => String::new(),
        };
        text.replace_range(start..end, &replacement);
    }
    // Bloques separados por una sola línea en blanco, sin espacios sobrantes.
    let blocks: Vec<&str> = text
        .split("\n\n")
        .map(str::trim)
        .filter(|block| !block.is_empty())
        .collect();
    tables_to_markdown(&blocks.join("\n\n"))
}

/// Caja de una figura según el modelo, en milésimas del ancho y del alto de la página.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FigureBox {
    pub left: u32,
    pub top: u32,
    pub right: u32,
    pub bottom: u32,
}

const FIGURE_MARKER: &str = "[[liblevo-figura:";
/// Lado mínimo de una figura, en milésimas de la página (descarta motas y adornos).
const MIN_FIGURE_SIDE: u32 = 40;

impl FigureBox {
    /// Lee `images/bbox_{left}_{top}_{right}_{bottom}.jpg` de una etiqueta `<img>` del modelo.
    fn from_tag(tag: &str) -> Option<Self> {
        let start = tag.find("bbox_")? + "bbox_".len();
        let numbers: Vec<u32> = tag[start..]
            .split(['_', '.', '"', '\''])
            .take(4)
            .map(|n| n.parse().ok())
            .collect::<Option<_>>()?;
        let figure = Self {
            left: numbers[0].min(1000),
            top: numbers[1].min(1000),
            right: numbers[2].min(1000),
            bottom: numbers[3].min(1000),
        };
        (figure.right >= figure.left + MIN_FIGURE_SIDE
            && figure.bottom >= figure.top + MIN_FIGURE_SIDE)
            .then_some(figure)
    }

    fn marker(&self) -> String {
        format!(
            "{FIGURE_MARKER}{},{},{},{}]]",
            self.left, self.top, self.right, self.bottom
        )
    }

    fn from_marker(block: &str) -> Option<Self> {
        let inner = block
            .trim()
            .strip_prefix(FIGURE_MARKER)?
            .strip_suffix("]]")?;
        let n: Vec<u32> = inner
            .split(',')
            .map(|v| v.parse().ok())
            .collect::<Option<_>>()?;
        (n.len() == 4).then(|| Self {
            left: n[0],
            top: n[1],
            right: n[2],
            bottom: n[3],
        })
    }
}

/// El texto releído sin las marcas de figura (para la guarda de aceptación).
pub fn without_figures(text: &str) -> String {
    text.split("\n\n")
        .filter(|block| FigureBox::from_marker(block).is_none())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Recorta de la imagen de la página cada figura marcada en `text`, la guarda como
/// `fig-p<página>-<n>.png` en `out_dir` y pone en su lugar su referencia `![](nombre)`. Devuelve el
/// texto y los archivos guardados; una figura que no se puede recortar se quita sin más.
pub fn crop_figures(
    page_image: &Path,
    page: u32,
    text: &str,
    out_dir: &Path,
) -> (String, Vec<PathBuf>) {
    let image = image::open(page_image).ok();
    let mut files = Vec::new();
    let blocks: Vec<String> = text
        .split("\n\n")
        .filter_map(|block| {
            let Some(figure) = FigureBox::from_marker(block) else {
                return Some(block.to_string());
            };
            let image = image.as_ref()?;
            let (w, h) = (image.width(), image.height());
            let x = figure.left * w / 1000;
            let y = figure.top * h / 1000;
            let cw = (figure.right * w / 1000).saturating_sub(x);
            let ch = (figure.bottom * h / 1000).saturating_sub(y);
            if cw == 0 || ch == 0 || fs::create_dir_all(out_dir).is_err() {
                return None;
            }
            let file = out_dir.join(format!("fig-p{page}-{}.png", files.len()));
            image.crop_imm(x, y, cw, ch).save(&file).ok()?;
            let name = file.file_name()?.to_string_lossy().into_owned();
            files.push(file);
            Some(format!("![]({name})"))
        })
        .collect();
    (blocks.join("\n\n"), files)
}

/// Solo las marcas de figura de un texto releído, para añadirlas a una página que conserva su OCR.
pub fn only_figures(text: &str) -> String {
    text.split("\n\n")
        .filter(|block| FigureBox::from_marker(block).is_some())
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn words(text: &str) -> std::collections::HashMap<String, usize> {
    let mut counts = std::collections::HashMap::new();
    for word in text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        *counts.entry(word.to_lowercase()).or_insert(0) += 1;
    }
    counts
}

/// Guarda de la relectura frente al OCR de la misma página.
pub fn accept(ocr: &str, reread: &str) -> bool {
    let (a, b) = (words(ocr), words(reread));
    let ocr_total: usize = a.values().sum();
    let reread_total: usize = b.values().sum();
    if ocr_total == 0 || reread_total == 0 {
        return false;
    }
    let kept: usize = a
        .iter()
        .map(|(w, n)| (*n).min(b.get(w).copied().unwrap_or(0)))
        .sum();
    kept as f64 / ocr_total as f64 >= MIN_KEPT
        && reread_total as f64 <= ocr_total as f64 * MAX_LENGTH_RATIO
}

/// Sustituye el texto de una página del Markdown con marcas `<!-- page: N -->`.
pub fn replace_page(markdown: &str, page: u32, text: &str) -> String {
    let marker = format!("<!-- page: {page} -->");
    let Some(start) = markdown.find(&marker).map(|s| s + marker.len()) else {
        return markdown.to_string();
    };
    let end = markdown[start..]
        .find("<!-- page:")
        .map_or(markdown.len(), |e| start + e);
    format!(
        "{}\n\n{}\n\n{}",
        &markdown[..start],
        text.trim(),
        &markdown[end..]
    )
}

/// Palabras del OCR de cada página (para fijar el límite de salida).
pub fn ocr_words(markdown: &str, page: u32) -> usize {
    markdown_pages(markdown)
        .get(&page)
        .map_or(0, |t| words(t).values().sum())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OCR: &str = "The specific gravity of marcasite is lower than that of pyrite and its \
        colour is paler when freshly broken, the crystals are orthorhombic";

    #[test]
    fn good_reread_is_accepted_and_runaway_or_cut_ones_are_not() {
        let fixed = OCR.replace("marcasite", "MARCASITE");
        assert!(accept(
            OCR,
            &format!("{fixed}\n\n| A | B |\n| --- | --- |\n| 1 | 2 |")
        ));
        // Corte: falta más de una cuarta parte del texto.
        let cut: String = OCR.split(' ').take(10).collect::<Vec<_>>().join(" ");
        assert!(!accept(OCR, &cut));
        // Bucle: el mismo texto repetido muchas veces.
        assert!(!accept(OCR, &OCR.repeat(3)));
        assert!(!accept(OCR, ""));
    }

    #[test]
    fn model_output_is_cleaned() {
        let raw = "<think>razonando</think>\n# Título\n<img src=\"images/bbox_1_2_3_4.jpg\" />\n\
            <table><tr><td>a</td><td>b</td></tr></table>";
        let cleaned = clean(raw);
        // La caja de 2 × 2 milésimas es una mota: no queda marca de figura.
        assert!(!cleaned.contains("think") && !cleaned.contains("<img"));
        assert!(!cleaned.contains("liblevo-figura"));
        assert!(cleaned.contains("# Título") && cleaned.contains("| a | b |"));
    }

    #[test]
    fn figures_are_marked_once_and_cropped_from_the_page() {
        let tag = "<img src=\"images/bbox_100_200_600_700.jpg\" />";
        let cleaned = clean(&format!("Texto antes.\n{tag}\nPie de figura.\n{tag}"));
        assert_eq!(cleaned.matches("liblevo-figura").count(), 1);
        assert_eq!(without_figures(&cleaned), "Texto antes.\n\nPie de figura.");
        assert_eq!(only_figures(&cleaned), "[[liblevo-figura:100,200,600,700]]");
        let dir = std::env::temp_dir().join(format!("liblevo-figuras-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let page = dir.join("page.png");
        image::RgbImage::from_pixel(1000, 2000, image::Rgb([255, 255, 255]))
            .save(&page)
            .unwrap();
        let (text, files) = crop_figures(&page, 7, &cleaned, &dir.join("images"));
        assert_eq!(text, "Texto antes.\n\n![](fig-p7-0.png)\n\nPie de figura.");
        let crop = image::open(&files[0]).unwrap();
        assert_eq!((crop.width(), crop.height()), (500, 1000));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn only_the_given_page_is_replaced() {
        let doc =
            "<!-- page: 1 -->\n\nUno\n\n<!-- page: 2 -->\n\nDos mal\n\n<!-- page: 3 -->\n\nTres";
        let out = replace_page(doc, 2, "Dos bien");
        assert_eq!(
            out,
            "<!-- page: 1 -->\n\nUno\n\n<!-- page: 2 -->\n\nDos bien\n\n<!-- page: 3 -->\n\nTres"
        );
        assert_eq!(replace_page(doc, 9, "x"), doc);
        assert_eq!(ocr_words(doc, 2), 2);
    }

    #[test]
    fn output_limit_follows_the_page_size() {
        assert_eq!(max_tokens_for(10), 1024);
        assert_eq!(max_tokens_for(400), 2400);
        assert_eq!(max_tokens_for(5000), 4096);
    }

    #[test]
    fn unverified_program_is_rejected() {
        let dir = std::env::temp_dir().join(format!("liblevo-visual-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let reader = VisualReader {
            program_dir: dir.clone(),
            model_dir: dir.clone(),
        };
        assert!(matches!(reader.verify(), Err(ReaderError::Unverified(_))));
        fs::remove_dir_all(&dir).unwrap();
    }
}
