//! Lector Xberg como programa oficial aparte.
//!
//! Liblevo ejecuta el binario oficial de Xberg 1.3.10 para Windows (MIT) en un proceso separado,
//! sin red, con CPU y cuatro hilos. Antes de cada uso comprueba la huella SHA-256 de sus tres
//! archivos; un binario distinto se rechaza. Si Xberg falla o se cancela, la app sigue viva.

use sha2::{Digest, Sha256};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Archivos del binario oficial de Xberg 1.3.10 para Windows x64 y sus huellas.
pub const XBERG_FILES: [(&str, &str); 3] = [
    (
        "xberg.exe",
        "54a8cfd31346813e2fcc968e9df53176d626f85e712ddccdd331d7fdfd0cfa69",
    ),
    (
        "onnxruntime.dll",
        "114947d633e6844ce3c4b51ef6678f776628571d08a5763859c61642c8dcca9c",
    ),
    (
        "onnxruntime_providers_shared.dll",
        "c10763a3764f5c292072533560c341565be64f38fd2807ed5035cd177d3bbb26",
    ),
];

const PAGE_MARKER_FORMAT: &str = "<!-- page: {page_num} -->";

#[derive(Debug, PartialEq, Eq)]
pub enum ReaderError {
    /// Falta un archivo del lector o su huella no coincide.
    Unverified(String),
    Cancelled,
    TimedOut,
    /// El lector terminó con error o no produjo salida.
    Failed,
    /// El programa no llegó a arrancar (falta una biblioteca o no se pudo preparar).
    NotStarted,
    Io,
}

impl ReaderError {
    pub fn message(&self) -> String {
        match self {
            Self::Unverified(file) => {
                format!("El componente de lectura no está verificado ({file}).")
            }
            Self::Cancelled => "La lectura se canceló.".into(),
            Self::TimedOut => "La lectura superó el tiempo máximo.".into(),
            Self::Failed => "El lector no pudo procesar el documento.".into(),
            Self::NotStarted => {
                "El programa no pudo arrancar. Vuelve a descargar el componente desde Componentes."
                    .into()
            }
            Self::Io => "No se pudo leer o escribir un archivo temporal.".into(),
        }
    }
}

pub struct XbergReader {
    /// Carpeta con `xberg.exe` y sus bibliotecas.
    pub program_dir: PathBuf,
    /// Carpeta con los modelos de maquetación y OCR ya preparados (caché de Hugging Face).
    pub models_dir: PathBuf,
}

pub struct ReadRequest<'a> {
    pub pdf: &'a Path,
    /// Carpeta privada del intento; el lector escribe aquí y en ningún otro sitio.
    pub staging: &'a Path,
    /// Forzar OCR de página completa (escaneos) en lugar de usar el texto del PDF.
    pub force_ocr: bool,
    pub timeout: Duration,
}

/// Lectura de un documento que no es PDF.
pub struct ReadOther {
    pub markdown: String,
    pub images: Vec<PathBuf>,
}

pub struct ReadOutput {
    /// Markdown con una marca `<!-- page: N -->` al empezar cada página.
    pub markdown: String,
}

pub fn sha256_file(path: &Path) -> Result<String, ReaderError> {
    let mut file = fs::File::open(path).map_err(|_| ReaderError::Io)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer).map_err(|_| ReaderError::Io)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

impl XbergReader {
    /// Comprueba que los tres archivos del lector son exactamente los fijados.
    pub fn verify(&self) -> Result<(), ReaderError> {
        for (name, expected) in XBERG_FILES {
            let path = self.program_dir.join(name);
            match sha256_file(&path) {
                Ok(actual) if actual == expected => {}
                _ => return Err(ReaderError::Unverified(name.to_string())),
            }
        }
        Ok(())
    }

    fn arguments(&self, request: &ReadRequest, output: &Path) -> Vec<String> {
        let config = serde_json::json!({
            "pages": {
                "extract_pages": true,
                "insert_page_markers": true,
                "marker_format": PAGE_MARKER_FORMAT,
            },
            "ocr": {
                "backend": "paddle-ocr",
                "language": ["en"],
                "auto_rotate": false,
                "vlm_fallback": {"mode": "disabled"},
                "paddle_ocr_settings": {
                    "language": "en",
                    "model_tier": "small",
                    "model_version": "pp-ocrv6",
                    "inference_backend": "ort",
                    "use_angle_cls": false,
                },
            },
        });
        let mut arguments: Vec<String> = [
            "extract",
            "--no-config-discovery",
            "--config-json",
            &config.to_string(),
            "--content-format",
            "markdown",
            "--no-cache",
            "true",
            "--layout",
            "--layout-table-model",
            "tatr",
            "--acceleration",
            "cpu",
            "--max-threads",
            "4",
            "--log-level",
            "error",
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        if request.force_ocr {
            arguments.extend(
                [
                    "--ocr",
                    "true",
                    "--ocr-backend",
                    "paddle-ocr",
                    "--force-ocr",
                    "true",
                ]
                .map(String::from),
            );
            // Medido el 19 de octubre de 2026: en escaneos, el modo de Xberg con extracción de
            // imágenes da mejor texto (Marianela, Capitán Veneno, s4). Las imágenes se descartan:
            // en un escaneo son las propias páginas.
            arguments.extend([
                "--extract-images".into(),
                "true".into(),
                "--output-dir".into(),
            ]);
            arguments.push(
                request
                    .staging
                    .join("discarded-images")
                    .to_string_lossy()
                    .into_owned(),
            );
        }
        arguments.push(request.pdf.to_string_lossy().into_owned());
        arguments.push("-o".into());
        arguments.push(output.to_string_lossy().into_owned());
        arguments
    }

    /// Lee un PDF. `cancel` detiene el proceso del lector en cuanto se activa.
    pub fn read(
        &self,
        request: &ReadRequest,
        cancel: &AtomicBool,
    ) -> Result<ReadOutput, ReaderError> {
        self.verify()?;
        let discarded = request.staging.join("discarded-images");
        fs::create_dir_all(&discarded).map_err(|_| ReaderError::Io)?;
        let output = request.staging.join("xberg.md");
        let _ = fs::remove_file(&output);
        self.run(
            self.arguments(request, &output),
            request.staging,
            request.timeout,
            cancel,
        )?;
        let _ = fs::remove_dir_all(&discarded);
        let raw = fs::read_to_string(&output).map_err(|_| ReaderError::Failed)?;
        Ok(ReadOutput {
            markdown: normalize_markers(&raw),
        })
    }
    /// Ejecuta Xberg con `arguments`, sin red, atado a la app, con tiempo máximo y cancelación.
    fn run(
        &self,
        arguments: Vec<String>,
        staging: &Path,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> Result<(), ReaderError> {
        let models = self.models_dir.to_string_lossy().into_owned();
        let mut command = Command::new(self.program_dir.join("xberg.exe"));
        command
            .args(arguments)
            .env_clear()
            .envs(std::env::vars().filter(|(k, _)| {
                matches!(
                    k.to_uppercase().as_str(),
                    "SYSTEMROOT"
                        | "WINDIR"
                        | "TEMP"
                        | "TMP"
                        | "PATH"
                        | "PATHEXT"
                        | "COMSPEC"
                        | "NUMBER_OF_PROCESSORS"
                        | "LOCALAPPDATA"
                        | "USERPROFILE"
                )
            }))
            .env("HF_HUB_OFFLINE", "1")
            .env("HF_HUB_CACHE", &models)
            .env("HUGGINGFACE_HUB_CACHE", &models)
            .env("XBERG_OCR_MODEL_CACHE_DIR", &models)
            .env("XBERG_CACHE_DIR", staging.join("cache"))
            .env("OMP_NUM_THREADS", "4")
            .env("RAYON_NUM_THREADS", "4")
            .env("CUDA_VISIBLE_DEVICES", "")
            .env("RUST_LOG", "off")
            .env("DO_NOT_TRACK", "1")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
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
        if !status.success() {
            return Err(ReaderError::Failed);
        }
        Ok(())
    }

    /// Lee un documento que no es PDF (DOCX, EPUB, HTML, TXT…) con sus imágenes, que deja en la
    /// carpeta `images` del área privada.
    pub fn read_other(
        &self,
        path: &Path,
        staging: &Path,
        timeout: Duration,
        cancel: &AtomicBool,
    ) -> Result<ReadOther, ReaderError> {
        self.verify()?;
        let images = staging.join("images");
        fs::create_dir_all(&images).map_err(|_| ReaderError::Io)?;
        let output = staging.join("xberg.md");
        let _ = fs::remove_file(&output);
        let mut arguments: Vec<String> = [
            "extract",
            "--no-config-discovery",
            "--content-format",
            "markdown",
            "--no-cache",
            "true",
            "--max-threads",
            "4",
            "--log-level",
            "error",
            "--extract-images",
            "true",
            "--output-dir",
        ]
        .map(String::from)
        .to_vec();
        arguments.push(images.to_string_lossy().into_owned());
        arguments.push(path.to_string_lossy().into_owned());
        arguments.push("-o".into());
        arguments.push(output.to_string_lossy().into_owned());
        self.run(arguments, staging, timeout, cancel)?;
        let markdown = fs::read_to_string(&output)
            .map_err(|_| ReaderError::Failed)?
            .replace("\r\n", "\n");
        let mut found: Vec<PathBuf> = fs::read_dir(&images)
            .map_err(|_| ReaderError::Io)?
            .flatten()
            .map(|entry| entry.path())
            .collect();
        found.sort();
        Ok(ReadOther {
            markdown,
            images: found,
        })
    }
}

/// Deja cada marca de página en su propio bloque y unifica los saltos de línea.
pub fn normalize_markers(markdown: &str) -> String {
    let unix = markdown.replace("\r\n", "\n");
    let mut out = String::with_capacity(unix.len() + 64);
    let mut rest = unix.as_str();
    while let Some(start) = rest.find("<!-- page:") {
        let Some(end) = rest[start..].find("-->").map(|e| start + e + 3) else {
            break;
        };
        out.push_str(rest[..start].trim_end());
        out.push_str("\n\n");
        out.push_str(&rest[start..end]);
        out.push_str("\n\n");
        rest = rest[end..].trim_start();
    }
    out.push_str(rest);
    out.trim_start().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markers_get_their_own_blocks() {
        let raw = "<!-- page: 1 --># Título\r\nTexto\r\n<!-- page: 2 -->Más";
        assert_eq!(
            normalize_markers(raw),
            "<!-- page: 1 -->\n\n# Título\nTexto\n\n<!-- page: 2 -->\n\nMás"
        );
    }

    #[test]
    fn unverified_program_is_rejected_before_running() {
        let dir = std::env::temp_dir().join(format!("liblevo-xberg-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        for (name, _) in XBERG_FILES {
            fs::write(dir.join(name), b"no es el binario oficial").unwrap();
        }
        let reader = XbergReader {
            program_dir: dir.clone(),
            models_dir: dir.clone(),
        };
        assert_eq!(
            reader.verify(),
            Err(ReaderError::Unverified("xberg.exe".into()))
        );
        let request = ReadRequest {
            pdf: &dir.join("x.pdf"),
            staging: &dir.join("staging"),
            force_ocr: false,
            timeout: Duration::from_secs(5),
        };
        assert!(matches!(
            reader.read(&request, &AtomicBool::new(false)),
            Err(ReaderError::Unverified(_))
        ));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn force_ocr_adds_only_the_ocr_switches() {
        let reader = XbergReader {
            program_dir: PathBuf::from("x"),
            models_dir: PathBuf::from("m"),
        };
        let pdf = PathBuf::from("doc.pdf");
        let staging = PathBuf::from("s");
        let mut request = ReadRequest {
            pdf: &pdf,
            staging: &staging,
            force_ocr: false,
            timeout: Duration::from_secs(1),
        };
        let plain = reader.arguments(&request, Path::new("o.md"));
        assert!(!plain.contains(&"--force-ocr".to_string()));
        request.force_ocr = true;
        let forced = reader.arguments(&request, Path::new("o.md"));
        // Las imágenes de un PDF digital salen de PDFium: Xberg lee sin extraerlas.
        assert!(!plain.contains(&"--extract-images".to_string()));
        assert!(forced.contains(&"--extract-images".to_string()));
        assert!(forced.contains(&"--force-ocr".to_string()));
        assert_eq!(forced.len(), plain.len() + 10);
        assert_eq!(forced.last().unwrap(), "o.md");
    }
}
