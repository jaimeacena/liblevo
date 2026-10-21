//! Lectura mínima de PDF con PDFium: identidad, número de páginas y vista de una página.

use pdfium_render::prelude::*;
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Tamaño máximo admitido para un PDF de entrada.
const MAX_PDF_BYTES: u64 = 2 * 1024 * 1024 * 1024;

#[derive(Debug, PartialEq, Eq)]
pub enum PdfError {
    NotFound,
    TooLarge,
    NotPdf,
    Unreadable,
    PageOutOfRange,
    EngineUnavailable,
}

impl PdfError {
    pub fn message(&self) -> &'static str {
        match self {
            Self::NotFound => "No se encuentra el archivo.",
            Self::TooLarge => "El archivo supera el tamaño máximo admitido.",
            Self::NotPdf => "El archivo no es un PDF.",
            Self::Unreadable => "No se pudo leer el PDF.",
            Self::PageOutOfRange => "Esa página no existe.",
            Self::EngineUnavailable => "No se encuentra el componente PDFium.",
        }
    }
}

pub struct Inspected {
    pub sha256: String,
    pub pages: u32,
}

pub fn pdfium() -> Result<&'static Pdfium, PdfError> {
    static PDFIUM: OnceLock<Option<Pdfium>> = OnceLock::new();
    PDFIUM
        .get_or_init(|| {
            library_path()
                .and_then(|path| Pdfium::bind_to_library(path).ok())
                .map(Pdfium::new)
        })
        .as_ref()
        .ok_or(PdfError::EngineUnavailable)
}

/// `LIBLEVO_PDFIUM` durante el desarrollo; junto al ejecutable en la app instalada.
fn library_path() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("LIBLEVO_PDFIUM") {
        return Some(PathBuf::from(path));
    }
    let beside = std::env::current_exe().ok()?.with_file_name("pdfium.dll");
    beside.exists().then_some(beside)
}

/// Comprueba la cabecera, calcula la huella y cuenta las páginas sin modificar el original.
pub fn inspect(path: &Path) -> Result<Inspected, PdfError> {
    let metadata = std::fs::metadata(path).map_err(|_| PdfError::NotFound)?;
    if metadata.len() > MAX_PDF_BYTES {
        return Err(PdfError::TooLarge);
    }
    let mut file = std::fs::File::open(path).map_err(|_| PdfError::NotFound)?;
    let mut header = [0u8; 5];
    file.read_exact(&mut header).map_err(|_| PdfError::NotPdf)?;
    if &header != b"%PDF-" {
        return Err(PdfError::NotPdf);
    }
    let mut hasher = Sha256::new();
    hasher.update(header);
    let mut buffer = vec![0u8; 1 << 20];
    loop {
        let read = file.read(&mut buffer).map_err(|_| PdfError::Unreadable)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let sha256 = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let document = pdfium()?
        .load_pdf_from_file(path, None)
        .map_err(|_| PdfError::Unreadable)?;
    let pages = u32::try_from(document.pages().len()).map_err(|_| PdfError::Unreadable)?;
    Ok(Inspected { sha256, pages })
}

/// Vista PNG de una página (1-based) con la anchura indicada en píxeles.
pub fn render_png(path: &Path, page: u32, width: u32) -> Result<Vec<u8>, PdfError> {
    let document = pdfium()?
        .load_pdf_from_file(path, None)
        .map_err(|_| PdfError::Unreadable)?;
    let index = page
        .checked_sub(1)
        .and_then(|i| PdfPageIndex::try_from(i).ok())
        .ok_or(PdfError::PageOutOfRange)?;
    let pdf_page = document
        .pages()
        .get(index)
        .map_err(|_| PdfError::PageOutOfRange)?;
    let bitmap = pdf_page
        .render_with_config(
            &PdfRenderConfig::new()
                .set_target_width(i32::try_from(width).unwrap_or(1200))
                .render_form_data(true),
        )
        .map_err(|_| PdfError::Unreadable)?;
    let mut png = Vec::new();
    bitmap
        .as_image()
        .map_err(|_| PdfError::Unreadable)?
        .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
        .map_err(|_| PdfError::Unreadable)?;
    Ok(png)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_and_non_pdf_files() {
        let dir = std::env::temp_dir().join(format!("liblevo-app-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(inspect(&dir.join("no.pdf")).err(), Some(PdfError::NotFound));
        let fake = dir.join("falso.pdf");
        std::fs::write(&fake, b"no es un pdf").unwrap();
        assert_eq!(inspect(&fake).err(), Some(PdfError::NotPdf));
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// PDF mínimo de una página escrito a mano, con su tabla de referencias cruzadas.
    fn minimal_pdf() -> Vec<u8> {
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>",
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 100] >>",
        ];
        let mut pdf = b"%PDF-1.4
"
        .to_vec();
        let mut offsets = Vec::new();
        for (i, body) in objects.iter().enumerate() {
            offsets.push(pdf.len());
            pdf.extend(
                format!(
                    "{} 0 obj
{body}
endobj
",
                    i + 1
                )
                .as_bytes(),
            );
        }
        let xref = pdf.len();
        pdf.extend(
            format!(
                "xref
0 {}
0000000000 65535 f 
",
                objects.len() + 1
            )
            .as_bytes(),
        );
        for offset in offsets {
            pdf.extend(
                format!(
                    "{offset:010} 00000 n 
"
                )
                .as_bytes(),
            );
        }
        pdf.extend(
            format!(
                "trailer
<< /Size {} /Root 1 0 R >>
startxref
{xref}
%%EOF
",
                objects.len() + 1
            )
            .as_bytes(),
        );
        pdf
    }

    #[test]
    fn inspects_and_renders_a_real_pdf_when_pdfium_is_available() {
        if std::env::var_os("LIBLEVO_PDFIUM").is_none() {
            eprintln!("LIBLEVO_PDFIUM no está definido: se omite la prueba con PDFium");
            return;
        }
        let dir = std::env::temp_dir().join(format!("liblevo-app-pdf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("una-pagina.pdf");
        std::fs::write(&path, minimal_pdf()).unwrap();
        let inspected = inspect(&path).unwrap();
        assert_eq!(inspected.pages, 1);
        assert_eq!(inspected.sha256.len(), 64);
        let png = render_png(&path, 1, 400).unwrap();
        assert!(png.starts_with(b"\x89PNG"));
        assert_eq!(
            render_png(&path, 2, 400).err(),
            Some(PdfError::PageOutOfRange)
        );
        assert_eq!(
            render_png(&path, 0, 400).err(),
            Some(PdfError::PageOutOfRange)
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
