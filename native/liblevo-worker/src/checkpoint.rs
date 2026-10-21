//! Puntos de control de una conversión, para reanudarla tras un cierre o una cancelación.
//!
//! Se guardan las dos partes caras: la lectura completa de Xberg y el resultado de cada página
//! releída con el modelo visual (texto final de la página, figuras recortadas y segundos gastados).
//! Todo queda ligado a una identidad (documento, componentes y versión del proceso): si cambia,
//! los puntos de control anteriores se descartan. Cada archivo se escribe de forma atómica (temporal
//! y cambio de nombre), así que un cierre a mitad nunca deja uno a medias. La app los borra al
//! publicar el resultado.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Versión del proceso de conversión. Cambia cuando un punto de control antiguo ya no sirve.
pub const PROCESS_VERSION: &str = "f3-reanudacion-1";

const IDENTITY: &str = "identidad.txt";
const READ: &str = "lectura.md";
const PAGES: &str = "paginas";
const FIGURES: &str = "figuras";

/// Lo que dejó la relectura de una página.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PageRecord {
    pub page: u32,
    /// Texto final de la página (con sus referencias de figura), o `None` si conservó su OCR.
    pub text: Option<String>,
    /// El texto releído pasó la guarda (si no, `text` es el OCR con las figuras añadidas).
    pub accepted: bool,
    pub seconds: f64,
}

pub struct Checkpoint {
    dir: PathBuf,
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let temporary = path.with_extension("tmp");
    fs::write(&temporary, bytes)?;
    fs::rename(&temporary, path)
}

impl Checkpoint {
    /// Abre (o crea) los puntos de control de `dir` para `identity`. Si los que hay son de otra
    /// identidad, se borran.
    pub fn open(dir: &Path, identity: &str) -> std::io::Result<Self> {
        let current = fs::read_to_string(dir.join(IDENTITY)).ok();
        if current.as_deref() != Some(identity) {
            let _ = fs::remove_dir_all(dir);
            fs::create_dir_all(dir.join(PAGES))?;
            fs::create_dir_all(dir.join(FIGURES))?;
            write_atomic(&dir.join(IDENTITY), identity.as_bytes())?;
        }
        fs::create_dir_all(dir.join(PAGES))?;
        fs::create_dir_all(dir.join(FIGURES))?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    /// Carpeta donde se guardan las figuras recortadas (sobreviven a un cierre).
    pub fn figures_dir(&self) -> PathBuf {
        self.dir.join(FIGURES)
    }

    pub fn read(&self) -> Option<String> {
        fs::read_to_string(self.dir.join(READ)).ok()
    }

    pub fn save_read(&self, markdown: &str) -> std::io::Result<()> {
        write_atomic(&self.dir.join(READ), markdown.as_bytes())
    }

    pub fn page(&self, page: u32) -> Option<PageRecord> {
        let bytes = fs::read(self.dir.join(PAGES).join(format!("{page}.json"))).ok()?;
        serde_json::from_slice::<PageRecord>(&bytes)
            .ok()
            .filter(|record| record.page == page)
    }

    pub fn save_page(&self, record: &PageRecord) -> std::io::Result<()> {
        let bytes = serde_json::to_vec(record).map_err(std::io::Error::other)?;
        write_atomic(
            &self.dir.join(PAGES).join(format!("{}.json", record.page)),
            &bytes,
        )
    }
}

/// ¿Hay puntos de control en `dir`? (para que la app ofrezca reanudar).
pub fn exists(dir: &Path) -> bool {
    dir.join(IDENTITY).is_file()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("liblevo-checkpoint-{name}-{}", std::process::id()))
    }

    #[test]
    fn saved_parts_come_back_for_the_same_identity_only() {
        let dir = temp("identidad");
        let _ = fs::remove_dir_all(&dir);
        let checkpoint = Checkpoint::open(&dir, "doc-a|xberg-1").unwrap();
        assert!(exists(&dir));
        assert_eq!(checkpoint.read(), None);
        checkpoint.save_read("<!-- page: 1 -->\n\nTexto").unwrap();
        let record = PageRecord {
            page: 3,
            text: Some("Página releída".into()),
            accepted: true,
            seconds: 21.5,
        };
        checkpoint.save_page(&record).unwrap();
        fs::write(checkpoint.figures_dir().join("fig-p3-0.png"), b"png").unwrap();
        // Mismo documento y componentes: todo sigue ahí.
        let again = Checkpoint::open(&dir, "doc-a|xberg-1").unwrap();
        assert_eq!(again.read().as_deref(), Some("<!-- page: 1 -->\n\nTexto"));
        assert_eq!(again.page(3), Some(record));
        assert_eq!(again.page(4), None);
        assert!(again.figures_dir().join("fig-p3-0.png").is_file());
        // Otra identidad (por ejemplo, otro lector): se descarta todo.
        let other = Checkpoint::open(&dir, "doc-a|xberg-2").unwrap();
        assert_eq!(other.read(), None);
        assert_eq!(other.page(3), None);
        assert!(!other.figures_dir().join("fig-p3-0.png").exists());
        fs::remove_dir_all(&dir).unwrap();
        assert!(!exists(&dir));
    }

    #[test]
    fn a_damaged_page_record_is_ignored() {
        let dir = temp("danado");
        let _ = fs::remove_dir_all(&dir);
        let checkpoint = Checkpoint::open(&dir, "x").unwrap();
        fs::write(dir.join(PAGES).join("2.json"), b"{medio escrito").unwrap();
        assert_eq!(checkpoint.page(2), None);
        fs::remove_dir_all(&dir).unwrap();
    }
}
