//! Catálogo cerrado de componentes descargables y su instalación verificada.
//!
//! Solo se descargan componentes públicos de este catálogo, desde su origen oficial (GitHub y
//! Hugging Face), nunca contenido del usuario. Cada descarga tiene su tamaño y su SHA-256 fijados;
//! cada archivo instalado, también. Todo se prepara en una carpeta temporal y solo sustituye a la
//! instalación anterior cuando todo cuadra: un fallo o una cancelación no dejan nada a medias.

use liblevo_worker::translate::{MODEL_1_8B, MODEL_7B, program_files as translation_files};
use liblevo_worker::visual_reader::{MODEL_FILES, PROGRAM_FILES};
use liblevo_worker::xberg_reader::{XBERG_FILES, sha256_file};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

/// Un archivo instalado: ruta dentro de la carpeta de componentes y su SHA-256.
#[derive(Debug, Clone)]
pub struct Installed {
    pub path: String,
    pub sha256: String,
}

/// Una descarga del catálogo.
#[derive(Debug, Clone)]
pub enum Part {
    /// Un ZIP del que se extraen solo los archivos indicados (buscados por su nombre).
    Archive {
        url: String,
        sha256: String,
        size: u64,
        files: Vec<Installed>,
    },
    /// Un archivo que se instala tal cual.
    File {
        url: String,
        sha256: String,
        size: u64,
        file: Installed,
    },
}

impl Part {
    fn size(&self) -> u64 {
        match self {
            Part::Archive { size, .. } | Part::File { size, .. } => *size,
        }
    }

    fn installed(&self) -> Vec<&Installed> {
        match self {
            Part::Archive { files, .. } => files.iter().collect(),
            Part::File { file, .. } => vec![file],
        }
    }
}

#[derive(Debug, Clone)]
pub struct Component {
    pub id: String,
    pub name: String,
    pub description: String,
    pub license: String,
    pub required: bool,
    /// Carpetas de primer nivel que pertenecen al componente (se sustituyen enteras).
    pub folders: Vec<String>,
    pub parts: Vec<Part>,
}

impl Component {
    pub fn size(&self) -> u64 {
        self.parts.iter().map(Part::size).sum()
    }

    /// Todos sus archivos están en su sitio. La huella se comprueba al usarlos.
    pub fn installed(&self, root: &Path) -> bool {
        self.parts
            .iter()
            .flat_map(Part::installed)
            .all(|f| root.join(&f.path).is_file())
    }
}

fn installed(path: impl Into<String>, sha256: &str) -> Installed {
    Installed {
        path: path.into(),
        sha256: sha256.into(),
    }
}

const XBERG_ZIP: &str = "https://github.com/xberg-io/xberg/releases/download/v1.3.10/xberg-cli-x86_64-pc-windows-msvc.zip";
const LLAMA_ZIP: &str = "https://github.com/ggml-org/llama.cpp/releases/download/b10566/llama-b10566-bin-win-cpu-x64.zip";
const LAYOUT: &str =
    "models--xberg-io--layout-models/snapshots/c6bf493e2f7b0b9a29a5870da9880c14e20ff0a3";
const PADDLE: &str =
    "models--xberg-io--paddleocr-onnx-models/snapshots/bc5ec866cf0e798e667808dfa51b0ba8ad0dafc8";
const OVIS_REVISION: &str = "8a22290a0c42dbcf84739d6d4c2763f877494ae0";
const HY_MT2_7B_REVISION: &str = "ab8472660ac61fac25f1af43fac2599d52a8a775";
const HY_MT2_1_8B_REVISION: &str = "a0c709d9fac510f2c807aa3af52872340dc37a4a";
const LLAMA_ZIP_SHA256: &str = "594f822c829ea097e1f0528a2b5b9f9953dd00254b48c44438104ddfb87a1ab9";
const LLAMA_ZIP_SIZE: u64 = 18_582_008;

/// Modelo de Hugging Face, fijado a su revisión, en la estructura de caché que lee Xberg.
fn xberg_model(snapshot: &str, file: &str, sha256: &str, size: u64) -> Part {
    let (repository, revision) = snapshot
        .trim_start_matches("models--")
        .split_once("/snapshots/")
        .unwrap_or_default();
    Part::File {
        url: format!(
            "https://huggingface.co/{}/resolve/{revision}/{file}",
            repository.replace("--", "/")
        ),
        sha256: sha256.into(),
        size,
        file: installed(format!("xberg-modelos/{snapshot}/{file}"), sha256),
    }
}

/// El catálogo: «lectura» (obligatorio), «relectura visual», «traducción» y «traducción de más
/// calidad» (opcionales). Huellas comprobadas el 19 y 20 de octubre de 2026 contra las publicadas
/// por GitHub y Hugging Face.
pub fn catalog() -> Vec<Component> {
    let mut xberg_files: Vec<Installed> = XBERG_FILES
        .iter()
        .map(|(name, hash)| installed(format!("xberg/{name}"), hash))
        .collect();
    xberg_files.push(installed(
        "xberg/LICENSE",
        "2df0cbac46062748fa0f582c5717df32d66607de4d37af1473ce785c31b60b54",
    ));
    xberg_files.push(installed(
        "xberg/THIRD_PARTY_LICENSES.md",
        "7f9a61b4ead9e7f0551fe9a1c2ffc1fb53f001e9170416412c17cdec93a0b6bd",
    ));
    let mut llama_files: Vec<Installed> = PROGRAM_FILES
        .iter()
        .map(|(name, hash)| installed(format!("ovis/{name}"), hash))
        .collect();
    llama_files.push(installed(
        "ovis/LICENSE-LLVM-OpenMP",
        "fdad1758a9e1f9d5a81e18879b3406772115edc92c24bfa36b70c654f325e8e4",
    ));
    let ovis_model = |(name, hash): (&str, &str), size: u64| Part::File {
        url: format!("https://huggingface.co/Abiray/OvisOCR2-GGUF/resolve/{OVIS_REVISION}/{name}"),
        sha256: hash.into(),
        size,
        file: installed(format!("ovis-modelo/{name}"), hash),
    };
    let mut translation_program: Vec<Installed> = translation_files()
        .iter()
        .map(|(name, hash)| installed(format!("traduccion/{name}"), hash))
        .collect();
    translation_program.push(installed(
        "traduccion/LICENSE-LLVM-OpenMP",
        "fdad1758a9e1f9d5a81e18879b3406772115edc92c24bfa36b70c654f325e8e4",
    ));
    let hy_mt2 =
        |repository: &str, revision: &str, (name, hash): (&str, &str), folder: &str, size| {
            Part::File {
                url: format!(
                    "https://huggingface.co/tencent/{repository}/resolve/{revision}/{name}"
                ),
                sha256: hash.into(),
                size,
                file: installed(format!("{folder}/{name}"), hash),
            }
        };
    vec![
        Component {
            id: "lectura".into(),
            name: "Lectura de PDF".into(),
            description: "Xberg 1.3.10 y sus modelos de maquetación, tablas y OCR. Necesario para \
                convertir."
                .into(),
            license: "MIT (Xberg) y Apache-2.0 (modelos)".into(),
            required: true,
            folders: vec!["xberg".into(), "xberg-modelos".into()],
            parts: vec![
                Part::Archive {
                    url: XBERG_ZIP.into(),
                    sha256: "488b83bf857da96e24f28c64b4cd55a3d530f6f55e22421717650ea38aff63c3"
                        .into(),
                    size: 50_542_957,
                    files: xberg_files,
                },
                xberg_model(
                    LAYOUT,
                    "rtdetr/model.onnx",
                    "3bf2fb0ee6df87435b7ae47f0f3930ec3dc97ec56fd824acc6d57bc7a6b89ef2",
                    169_089_059,
                ),
                xberg_model(
                    LAYOUT,
                    "tatr/model.onnx",
                    "c11f4033da75e9c4d41c403ef356e89caa0a37a7d111b55461e7d5ba856bb6b6",
                    30_158_413,
                ),
                xberg_model(
                    PADDLE,
                    "v2/classifiers/PP-LCNet_x1_0_textline_ori.onnx",
                    "1090f9f483a115f904beefe04acc9d28edf0c0b7b08cf0dd8d0ea59a9e0f2735",
                    6_775_212,
                ),
                xberg_model(
                    PADDLE,
                    "v6/det/small/model.onnx",
                    "b1a4f07289eda88d29239890b94ea2f9e29f5635a33ff6e165bb1b27dcea25fc",
                    9_893_093,
                ),
                xberg_model(
                    PADDLE,
                    "v6/rec/small/model.onnx",
                    "0cc734c53747ea66968d286f6e6597fc1c91df121964ab4be5085c156fcac892",
                    21_218_539,
                ),
                xberg_model(
                    PADDLE,
                    "v6/rec/small/dict.txt",
                    "b5f2bfe2bdd9448429e3e82b51c789775d9b42f2403d082b00662eb77e401c5d",
                    74_947,
                ),
            ],
        },
        Component {
            id: "relectura".into(),
            name: "Relectura visual".into(),
            description: "Modelo OvisOCR2 con llama.cpp b10566. Relee las páginas dudosas de los \
                escaneos y recorta sus figuras. Opcional."
                .into(),
            license: "MIT (llama.cpp) y Apache-2.0 (OvisOCR2)".into(),
            required: false,
            folders: vec!["ovis".into(), "ovis-modelo".into()],
            parts: vec![
                Part::Archive {
                    url: LLAMA_ZIP.into(),
                    sha256: LLAMA_ZIP_SHA256.into(),
                    size: LLAMA_ZIP_SIZE,
                    files: llama_files,
                },
                ovis_model(MODEL_FILES[0], 1_516_744_617),
                ovis_model(MODEL_FILES[1], 204_987_079),
            ],
        },
        Component {
            id: "traduccion".into(),
            name: "Traducción".into(),
            description: "Modelo Hy-MT2 1,8B con llama.cpp b10566. Traduce los documentos en el \
                equipo, bloque a bloque. Opcional."
                .into(),
            license: "MIT (llama.cpp) y Apache-2.0 (Hy-MT2)".into(),
            required: false,
            folders: vec!["traduccion".into(), "traduccion-rapida".into()],
            parts: vec![
                Part::Archive {
                    url: LLAMA_ZIP.into(),
                    sha256: LLAMA_ZIP_SHA256.into(),
                    size: LLAMA_ZIP_SIZE,
                    files: translation_program,
                },
                hy_mt2(
                    "Hy-MT2-1.8B-GGUF",
                    HY_MT2_1_8B_REVISION,
                    MODEL_1_8B,
                    "traduccion-rapida",
                    1_133_080_448,
                ),
            ],
        },
        Component {
            id: "traduccion-calidad".into(),
            name: "Traducción de más calidad".into(),
            description: "Modelo Hy-MT2 7B: traduce mejor, unas tres veces más despacio. Necesita \
                también «Traducción». Opcional."
                .into(),
            license: "Apache-2.0 (Hy-MT2)".into(),
            required: false,
            folders: vec!["traduccion-calidad".into()],
            parts: vec![hy_mt2(
                "Hy-MT2-7B-GGUF",
                HY_MT2_7B_REVISION,
                MODEL_7B,
                "traduccion-calidad",
                4_624_648_896,
            )],
        },
    ]
}

#[derive(Debug, PartialEq, Eq)]
pub enum InstallError {
    Cancelled,
    Network,
    /// La descarga o un archivo no tienen la huella o el tamaño fijados.
    Verification,
    Io,
}

impl InstallError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::Cancelled => "Descarga cancelada.",
            Self::Network => "No se pudo descargar. Comprueba la conexión e inténtalo de nuevo.",
            Self::Verification => {
                "Lo descargado no coincide con lo publicado; no se ha instalado nada."
            }
            Self::Io => "No se pudo guardar el componente en este equipo.",
        }
    }
}

/// Descarga `url` en `target` comprobando tamaño y SHA-256. `progress` recibe los bytes de esta
/// descarga a medida que llegan.
fn download(
    agent: &ureq::Agent,
    url: &str,
    sha256: &str,
    size: u64,
    target: &Path,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64),
) -> Result<(), InstallError> {
    let mut response = agent.get(url).call().map_err(|_| InstallError::Network)?;
    // Nunca se lee más de lo esperado: un archivo distinto no puede llenar el disco.
    let mut reader = response.body_mut().with_config().limit(size + 1).reader();
    let mut file = fs::File::create(target).map_err(|_| InstallError::Io)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; 1 << 20];
    let mut received = 0u64;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(InstallError::Cancelled);
        }
        let read = match reader.read(&mut buffer) {
            Ok(read) => read,
            Err(_) if received >= size => return Err(InstallError::Verification),
            Err(_) => return Err(InstallError::Network),
        };
        if read == 0 {
            break;
        }
        received += read as u64;
        if received > size {
            return Err(InstallError::Verification);
        }
        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read])
            .map_err(|_| InstallError::Io)?;
        progress(read as u64);
    }
    file.sync_all().map_err(|_| InstallError::Io)?;
    if received != size || format!("{:x}", hasher.finalize()) != sha256 {
        return Err(InstallError::Verification);
    }
    Ok(())
}

/// Extrae del ZIP cada archivo esperado (buscado por su nombre, esté en la carpeta que esté).
fn extract(archive: &Path, files: &[Installed], staging: &Path) -> Result<(), InstallError> {
    let file = fs::File::open(archive).map_err(|_| InstallError::Io)?;
    let mut zip = zip::ZipArchive::new(file).map_err(|_| InstallError::Verification)?;
    let names: Vec<String> = zip.file_names().map(str::to_string).collect();
    for wanted in files {
        let base = Path::new(&wanted.path)
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let matches: Vec<&String> = names
            .iter()
            .filter(|n| n.rsplit('/').next() == Some(base.as_str()))
            .collect();
        let [entry_name] = matches.as_slice() else {
            return Err(InstallError::Verification);
        };
        let mut entry = zip
            .by_name(entry_name)
            .map_err(|_| InstallError::Verification)?;
        let target = staging.join(&wanted.path);
        fs::create_dir_all(target.parent().unwrap_or(staging)).map_err(|_| InstallError::Io)?;
        let mut out = fs::File::create(&target).map_err(|_| InstallError::Io)?;
        std::io::copy(&mut entry, &mut out).map_err(|_| InstallError::Verification)?;
    }
    Ok(())
}

/// Descarga, verifica e instala un componente en `root`. `progress(hechos, total)` en bytes.
pub fn install(
    root: &Path,
    component: &Component,
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(u64, u64),
) -> Result<(), InstallError> {
    let staging = root.join(format!(".instalando-{}", component.id));
    let _ = fs::remove_dir_all(&staging);
    fs::create_dir_all(&staging).map_err(|_| InstallError::Io)?;
    let outcome = (|| {
        let agent: ureq::Agent = ureq::Agent::config_builder()
            .user_agent("Liblevo")
            .build()
            .into();
        let total = component.size();
        let mut done = 0u64;
        for part in &component.parts {
            let temporary = staging.join("descarga.tmp");
            match part {
                Part::Archive {
                    url,
                    sha256,
                    size,
                    files,
                } => {
                    download(&agent, url, sha256, *size, &temporary, cancel, &mut |n| {
                        done += n;
                        progress(done, total);
                    })?;
                    extract(&temporary, files, &staging)?;
                    fs::remove_file(&temporary).map_err(|_| InstallError::Io)?;
                }
                Part::File {
                    url,
                    sha256,
                    size,
                    file,
                } => {
                    download(&agent, url, sha256, *size, &temporary, cancel, &mut |n| {
                        done += n;
                        progress(done, total);
                    })?;
                    let target = staging.join(&file.path);
                    fs::create_dir_all(target.parent().unwrap_or(&staging))
                        .map_err(|_| InstallError::Io)?;
                    fs::rename(&temporary, &target).map_err(|_| InstallError::Io)?;
                }
            }
        }
        // Cada archivo instalado, con su huella.
        for file in component.parts.iter().flat_map(Part::installed) {
            if sha256_file(&staging.join(&file.path)).ok().as_deref() != Some(&file.sha256) {
                return Err(InstallError::Verification);
            }
        }
        replace_folders(root, &staging, &component.folders)
    })();
    let _ = fs::remove_dir_all(&staging);
    outcome
}

/// Sustituye cada carpeta del componente por la preparada; si algo falla, deja las anteriores.
fn replace_folders(root: &Path, staging: &Path, folders: &[String]) -> Result<(), InstallError> {
    let mut moved: Vec<(PathBuf, PathBuf)> = Vec::new();
    let rollback = |moved: &[(PathBuf, PathBuf)]| {
        for (current, old) in moved.iter().rev() {
            let _ = fs::remove_dir_all(current);
            if old.exists() {
                let _ = fs::rename(old, current);
            }
        }
    };
    for folder in folders {
        let current = root.join(folder);
        let old = root.join(format!(".anterior-{folder}"));
        let _ = fs::remove_dir_all(&old);
        if current.exists() && fs::rename(&current, &old).is_err() {
            rollback(&moved);
            return Err(InstallError::Io);
        }
        moved.push((current.clone(), old));
        if fs::rename(staging.join(folder), &current).is_err() {
            rollback(&moved);
            return Err(InstallError::Io);
        }
    }
    for (_, old) in moved {
        let _ = fs::remove_dir_all(old);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::TcpListener;

    /// Servidor HTTP mínimo que sirve unos archivos fijos (sin conexión a internet).
    fn serve(files: Vec<(&'static str, Vec<u8>)>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut stream = stream;
                let mut request = [0u8; 2048];
                let n = stream.read(&mut request).unwrap_or(0);
                let line = String::from_utf8_lossy(&request[..n]).to_string();
                let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
                match files.iter().find(|(p, _)| *p == path) {
                    Some((_, body)) => {
                        let _ = write!(
                            stream,
                            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            body.len()
                        );
                        let _ = stream.write_all(body);
                    }
                    None => {
                        let _ = stream.write_all(
                            b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                        );
                    }
                }
            }
        });
        format!("http://{address}")
    }

    fn sha(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn zip_with(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buffer = std::io::Cursor::new(Vec::new());
        {
            let mut zip = zip::ZipWriter::new(&mut buffer);
            for (name, body) in entries {
                zip.start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(body).unwrap();
            }
            zip.finish().unwrap();
        }
        buffer.into_inner()
    }

    fn component(base: &str, archive: &[u8], model: &[u8], model_hash: String) -> Component {
        Component {
            id: "prueba".into(),
            name: "Prueba".into(),
            description: String::new(),
            license: "MIT".into(),
            required: true,
            folders: vec!["programa".into(), "modelo".into()],
            parts: vec![
                Part::Archive {
                    url: format!("{base}/programa.zip"),
                    sha256: sha(archive),
                    size: archive.len() as u64,
                    files: vec![installed("programa/lector.exe", &sha(b"binario"))],
                },
                Part::File {
                    url: format!("{base}/modelo.bin"),
                    sha256: model_hash.clone(),
                    size: model.len() as u64,
                    file: installed("modelo/pesos.bin", &model_hash),
                },
            ],
        }
    }

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("liblevo-comp-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn verified_downloads_are_installed_and_replace_the_old_version() {
        let archive = zip_with(&[("paquete/lector.exe", b"binario"), ("paquete/LEEME", b"x")]);
        let base = serve(vec![
            ("/programa.zip", archive.clone()),
            ("/modelo.bin", b"pesos".to_vec()),
        ]);
        let root = temp("ok");
        fs::create_dir_all(root.join("programa")).unwrap();
        fs::write(root.join("programa/viejo.exe"), b"viejo").unwrap();
        let c = component(&base, &archive, b"pesos", sha(b"pesos"));
        assert!(!c.installed(&root));
        let mut last = (0, 0);
        install(&root, &c, &AtomicBool::new(false), &mut |d, t| {
            last = (d, t)
        })
        .unwrap();
        assert!(c.installed(&root));
        assert_eq!(
            fs::read(root.join("programa/lector.exe")).unwrap(),
            b"binario"
        );
        // Solo lo esperado: ni el archivo sobrante del ZIP ni la versión anterior.
        assert!(!root.join("programa/LEEME").exists());
        assert!(!root.join("programa/viejo.exe").exists());
        assert_eq!(last, (c.size(), c.size()));
        assert!(!root.join(".instalando-prueba").exists());
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn a_wrong_download_installs_nothing_and_keeps_the_old_version() {
        let archive = zip_with(&[("lector.exe", b"binario")]);
        // El servidor entrega otro modelo (misma longitud, otra huella).
        let base = serve(vec![
            ("/programa.zip", archive.clone()),
            ("/modelo.bin", b"PESOS".to_vec()),
        ]);
        let root = temp("mal");
        fs::create_dir_all(root.join("programa")).unwrap();
        fs::write(root.join("programa/lector.exe"), b"anterior").unwrap();
        let c = component(&base, &archive, b"pesos", sha(b"pesos"));
        let result = install(&root, &c, &AtomicBool::new(false), &mut |_, _| {});
        assert_eq!(result, Err(InstallError::Verification));
        assert_eq!(
            fs::read(root.join("programa/lector.exe")).unwrap(),
            b"anterior"
        );
        assert!(!root.join("modelo").exists());
        assert!(!root.join(".instalando-prueba").exists());
        // Un archivo que falta en el servidor es un fallo de red; cancelar, una cancelación.
        let mut missing = c.clone();
        if let Part::Archive { url, .. } = &mut missing.parts[0] {
            *url = format!("{base}/no-existe.zip");
        }
        assert_eq!(
            install(&root, &missing, &AtomicBool::new(false), &mut |_, _| {}),
            Err(InstallError::Network)
        );
        assert_eq!(
            install(&root, &c, &AtomicBool::new(true), &mut |_, _| {}),
            Err(InstallError::Cancelled)
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn the_catalog_pins_every_file_and_only_official_sources() {
        for component in catalog() {
            assert!(component.size() > 0);
            for part in &component.parts {
                let url = match part {
                    Part::Archive { url, .. } | Part::File { url, .. } => url,
                };
                assert!(
                    url.starts_with("https://github.com/")
                        || url.starts_with("https://huggingface.co/"),
                    "{url}"
                );
                for file in part.installed() {
                    assert_eq!(file.sha256.len(), 64, "{}", file.path);
                    assert!(
                        component
                            .folders
                            .iter()
                            .any(|f| file.path.starts_with(&format!("{f}/")))
                    );
                }
            }
        }
    }
}
