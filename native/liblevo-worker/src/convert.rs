//! Conversión de un PDF: lectura con Xberg, estilo observado con PDFium y pulido propio.
//!
//! Es el mismo camino para la app y para `liblevo-eval`, de modo que lo medido es lo que usa la
//! persona. El resultado conserva las marcas de página para la revisión junto al original.

use crate::checkpoint::{self, Checkpoint, PageRecord};
use crate::doubt;
use crate::images;
use crate::pdf_images;
use crate::pdf_styles::{outline, page_texts, styled_lines};
use crate::polish::{PolishReport, polish};
use crate::review::{self, PageStates, ReviewItem};
use crate::scan_detect;
use crate::visual_reader::{self, VisualReader};
use crate::xberg_reader::{ReadRequest, ReaderError, XBERG_FILES, XbergReader, sha256_file};
use pdfium_render::prelude::Pdfium;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

pub struct Converted {
    /// Markdown con marcas `<!-- page: N -->`.
    pub markdown: String,
    pub images: Vec<PathBuf>,
    pub report: PolishReport,
    /// Se leyó como escaneo, con OCR en lugar del texto del PDF.
    pub scanned: bool,
    /// Páginas de un escaneo en las que el OCR y la capa invisible no coinciden (`doubt`).
    pub doubtful_pages: Vec<u32>,
    /// Páginas releídas con el modelo visual y aceptadas por su guarda.
    pub reread_pages: Vec<u32>,
    /// Páginas de un escaneo con probable figura (`doubt::figure_pages`).
    pub figure_pages: Vec<u32>,
    /// El modelo visual estaba disponible y verificado.
    pub visual_available: bool,
    /// Lista de revisión ordenada por riesgo (`review`).
    pub review: Vec<ReviewItem>,
    pub seconds: f64,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ConvertError {
    Reader(ReaderError),
    /// No se pudo leer el estilo del PDF (archivo dañado o PDFium ausente).
    Styles,
}

impl ConvertError {
    pub fn message(&self) -> String {
        match self {
            Self::Reader(error) => error.message(),
            Self::Styles => "No se pudo analizar el PDF.".into(),
        }
    }
}

/// Escala a la que se dibuja una página para el modelo visual (la medida con el conjunto).
const VISUAL_SCALE: f32 = 2.0;
/// Tiempo máximo de relectura de una página; pasado, la página conserva su OCR.
const VISUAL_PAGE_TIMEOUT: Duration = Duration::from_secs(180);

/// Tiempo de relectura que se concede a un documento: 20 s por página, con un mínimo de 5
/// minutos. Un libro de tablas puede releerse entero y un documento denso que el modelo lee mal no
/// multiplica sin fin la conversión. Las páginas se releen en orden: medido el 19 de octubre de
/// 2026, empezar por las más dudosas gastaba el presupuesto en las que el modelo no sabe leer.
fn reread_budget(pages: usize) -> Duration {
    Duration::from_secs((pages as u64 * 20).max(300))
}

/// Relectura con el modelo visual (solo en escaneos).
#[derive(Clone, Copy, Default)]
pub struct Visual<'a> {
    /// El lector visual, si está instalado.
    pub reader: Option<&'a VisualReader>,
    /// Lectura visual completa (opción del documento): todas las páginas, sin presupuesto.
    pub full: bool,
}

/// Convierte un PDF. Si la mayoría de sus páginas son escaneos (`scan_detect`), se lee con OCR.
pub fn convert_pdf(
    pdfium: &Pdfium,
    reader: &XbergReader,
    pdf: &Path,
    staging: &Path,
    checkpoint_dir: &Path,
    visual: Visual,
    cancel: &AtomicBool,
) -> Result<Converted, ConvertError> {
    let Visual {
        reader: visual,
        full: visual_full,
    } = visual;
    let started = Instant::now();
    let lines = styled_lines(pdfium, pdf, 1).map_err(|_| ConvertError::Styles)?;
    let scanned = scan_detect::vote(pdfium, pdf, &lines)
        .map_err(|_| ConvertError::Styles)?
        .is_scanned();
    let visual = visual.filter(|v| scanned && v.verify().is_ok());
    // Puntos de control ligados al documento, a los componentes y a la versión del proceso.
    let identity = format!(
        "{}|{}|xberg:{}|visual:{}|escaneo:{scanned}",
        checkpoint::PROCESS_VERSION,
        sha256_file(pdf).map_err(|_| ConvertError::Styles)?,
        XBERG_FILES[0].1,
        visual.map_or("no", |_| visual_reader::MODEL_FILES[0].1),
    );
    let checkpoint =
        Checkpoint::open(checkpoint_dir, &identity).map_err(|_| ConvertError::Styles)?;
    let read_markdown = match checkpoint.read() {
        Some(markdown) => markdown,
        None => {
            let read = reader
                .read(
                    &ReadRequest {
                        pdf,
                        staging,
                        force_ocr: scanned,
                        timeout: Duration::from_secs(2 * 3600),
                    },
                    cancel,
                )
                .map_err(ConvertError::Reader)?;
            checkpoint
                .save_read(&read.markdown)
                .map_err(|_| ConvertError::Styles)?;
            read.markdown
        }
    };
    // Las imágenes salen de PDFium (digitales) o de las figuras del modelo visual (escaneos): se
    // quita cualquier referencia que traiga el lector.
    let mut markdown_read =
        images::drop_references(&read_markdown, &images::all_referenced(&read_markdown));
    let images_dir = staging.join("images");
    // Escaneo: páginas dudosas y con probable figura, releídas con el modelo visual antes del
    // pulido.
    let mut doubtful_pages = Vec::new();
    let mut figure_pages = Vec::new();
    let mut reread_pages = Vec::new();
    let mut figures: Vec<PathBuf> = Vec::new();
    if scanned {
        doubtful_pages = doubt::doubtful_pages(&markdown_read, &page_texts(pdfium, pdf));
        let ink = pdf_images::ink_fractions(pdfium, pdf).map_err(|_| ConvertError::Styles)?;
        figure_pages = doubt::figure_pages(&ink, &doubt::words_per_page(&markdown_read));
    }
    if let Some(visual) = visual {
        let page_count = doubt::markdown_pages(&markdown_read).len();
        let mut to_read: Vec<u32> = if visual_full {
            (1..=page_count as u32).collect()
        } else {
            doubtful_pages
                .iter()
                .chain(&figure_pages)
                .copied()
                .collect()
        };
        to_read.sort_unstable();
        to_read.dedup();
        let work = staging.join("visual");
        let budget = if visual_full {
            Duration::MAX
        } else {
            reread_budget(page_count)
        };
        let reread_started = Instant::now();
        // Tiempo de relectura ya gastado en intentos anteriores (cuenta para el presupuesto).
        let mut spent_before = Duration::ZERO;
        for page in to_read {
            if let Some(record) = checkpoint.page(page) {
                spent_before += Duration::from_secs_f64(record.seconds.max(0.0));
                if let Some(text) = &record.text {
                    markdown_read = visual_reader::replace_page(&markdown_read, page, text);
                }
                if record.accepted {
                    reread_pages.push(page);
                }
                continue;
            }
            // Presupuesto agotado: el resto de páginas conserva su OCR y sigue marcado.
            let Some(left) = budget.checked_sub(reread_started.elapsed() + spent_before) else {
                break;
            };
            let page_started = Instant::now();
            let image = work.join(format!("page-{page}.png"));
            std::fs::create_dir_all(&work).map_err(|_| ConvertError::Styles)?;
            if pdf_images::render_page(pdfium, pdf, page, VISUAL_SCALE, &image).is_err() {
                continue;
            }
            let ocr = visual_reader::ocr_words(&markdown_read, page);
            let raw = visual.read_page(
                &image,
                &work,
                visual_reader::max_tokens_for(ocr),
                VISUAL_PAGE_TIMEOUT.min(left),
                cancel,
            );
            let reread = match raw {
                Ok(raw) => visual_reader::clean(&raw),
                Err(ReaderError::Cancelled) => {
                    let _ = std::fs::remove_file(&image);
                    return Err(ConvertError::Reader(ReaderError::Cancelled));
                }
                // Un fallo o un tiempo agotado deja la página con su OCR (y no se repite).
                Err(_) => {
                    let _ = std::fs::remove_file(&image);
                    let _ = checkpoint.save_page(&PageRecord {
                        page,
                        text: None,
                        accepted: false,
                        seconds: page_started.elapsed().as_secs_f64(),
                    });
                    continue;
                }
            };
            let ocr_text = doubt::markdown_pages(&markdown_read)
                .remove(&page)
                .unwrap_or_default();
            let accepted =
                visual_reader::accept(&ocr_text, &visual_reader::without_figures(&reread));
            // Texto aceptado: la relectura entera, con sus figuras en su sitio. Si no, la página
            // conserva su OCR y, si es una página con figura, sus figuras van al final.
            let page_text = if accepted {
                Some(reread)
            } else if figure_pages.contains(&page) || visual_full {
                let only = visual_reader::only_figures(&reread);
                (!only.is_empty()).then(|| {
                    format!(
                        "{}

{only}",
                        ocr_text.trim()
                    )
                })
            } else {
                None
            };
            let mut final_text = None;
            if let Some(page_text) = page_text {
                let (text, _) = visual_reader::crop_figures(
                    &image,
                    page,
                    &page_text,
                    &checkpoint.figures_dir(),
                );
                markdown_read = visual_reader::replace_page(&markdown_read, page, &text);
                if accepted {
                    reread_pages.push(page);
                }
                final_text = Some(text);
            }
            let _ = std::fs::remove_file(&image);
            checkpoint
                .save_page(&PageRecord {
                    page,
                    text: final_text,
                    accepted,
                    seconds: page_started.elapsed().as_secs_f64(),
                })
                .map_err(|_| ConvertError::Styles)?;
        }
    }
    // Figuras recortadas (de este intento o de uno anterior) que usa el texto: al área del intento.
    if let Ok(entries) = std::fs::read_dir(checkpoint.figures_dir()) {
        std::fs::create_dir_all(&images_dir).map_err(|_| ConvertError::Styles)?;
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if markdown_read.contains(&format!("]({name})")) {
                let target = images_dir.join(&name);
                std::fs::copy(entry.path(), &target).map_err(|_| ConvertError::Styles)?;
                figures.push(target);
            }
        }
    }
    // El texto completo de las páginas solo es autoridad en un PDF digital: en un escaneo sería la
    // capa invisible, menos fiable que el OCR.
    let texts = if scanned {
        Default::default()
    } else {
        page_texts(pdfium, pdf)
    };
    let (polished, report) = polish(&markdown_read, &lines, &outline(pdfium, pdf), &texts);
    // PDF digital: imágenes incrustadas, con su página y su sitio.
    let placed = if scanned {
        Vec::new()
    } else {
        pdf_images::extract(pdfium, pdf, &images_dir).map_err(|_| ConvertError::Styles)?
    };
    let with_images = pdf_images::insert_references(&polished, &placed);
    // Solo se conservan las figuras: fuera logotipos repetidos, iconos y fondos.
    let rejected = images::rejected(&with_images, |name| images::facts(&images_dir.join(name)));
    let markdown = images::drop_references(&with_images, &rejected);
    let kept: Vec<PathBuf> = placed
        .into_iter()
        .map(|image| image.file)
        .chain(figures)
        .filter(|path| {
            let name = path.file_name().unwrap_or_default().to_string_lossy();
            !rejected.contains(name.as_ref()) && markdown.contains(&format!("]({name})"))
        })
        .collect();
    // Lista de revisión: referencia = texto de cada página según el PDF (en un escaneo, su capa
    // invisible, si la tiene).
    let review = review::review(
        &markdown,
        &page_texts(pdfium, pdf),
        &PageStates {
            doubtful: doubtful_pages.clone(),
            reread: reread_pages.clone(),
        },
    );
    Ok(Converted {
        review,
        markdown,
        images: kept,
        report,
        scanned,
        doubtful_pages,
        reread_pages,
        figure_pages,
        visual_available: visual.is_some(),
        seconds: started.elapsed().as_secs_f64(),
    })
}

/// Formatos que no son PDF y que se convierten. Todos se leen con Xberg salvo Markdown, que
/// ya es el formato del resultado.
pub const OTHER_FORMATS: [&str; 7] = ["docx", "epub", "html", "htm", "txt", "md", "markdown"];

/// ¿Se puede convertir este archivo? (PDF o uno de `OTHER_FORMATS`).
pub fn supported(path: &Path) -> bool {
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    extension == "pdf" || OTHER_FORMATS.contains(&extension.as_str())
}

/// Imágenes de un contenedor ZIP (EPUB, DOCX) a las que el texto hace referencia por su ruta dentro
/// del paquete («images/p1-0.jpeg», «../Images/a.png»): se copian del paquete original a
/// `images_dir` con un nombre plano y la referencia pasa a ese nombre. Las que no aparecen se dejan.
fn images_from_package(
    package: &Path,
    markdown: &str,
    images_dir: &Path,
) -> (String, Vec<PathBuf>) {
    let Ok(file) = std::fs::File::open(package) else {
        return (markdown.to_string(), Vec::new());
    };
    let Ok(mut zip) = zip::ZipArchive::new(file) else {
        return (markdown.to_string(), Vec::new());
    };
    let entries: Vec<String> = zip.file_names().map(str::to_string).collect();
    let mut out = markdown.to_string();
    let mut files = Vec::new();
    for reference in images::all_referenced(markdown) {
        let wanted = reference
            .split('/')
            .filter(|part| *part != ".." && *part != "." && !part.is_empty())
            .collect::<Vec<_>>()
            .join("/");
        if wanted.is_empty() {
            continue;
        }
        let matches: Vec<&String> = entries
            .iter()
            .filter(|entry| *entry == &wanted || entry.ends_with(&format!("/{wanted}")))
            .collect();
        let [entry_name] = matches.as_slice() else {
            continue;
        };
        let Ok(mut entry) = zip.by_name(entry_name) else {
            continue;
        };
        let flat = format!(
            "paq-{}-{}",
            files.len(),
            wanted.rsplit('/').next().unwrap_or("imagen")
        );
        let target = images_dir.join(&flat);
        let copied = std::fs::create_dir_all(images_dir).is_ok()
            && std::fs::File::create(&target)
                .ok()
                .is_some_and(|mut out| std::io::copy(&mut entry, &mut out).is_ok());
        if copied {
            out = out.replace(&format!("]({reference})"), &format!("]({flat})"));
            files.push(target);
        }
    }
    (out, files)
}

/// Convierte un documento que no es PDF. El resultado es una sola «página» (estos formatos no
/// tienen páginas fijas), pulido igual que un PDF y con su lista de revisión.
pub fn convert_other(
    reader: &XbergReader,
    path: &Path,
    staging: &Path,
    cancel: &AtomicBool,
) -> Result<Converted, ConvertError> {
    let started = Instant::now();
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    let images_dir = staging.join("images");
    let (body, found) = if extension == "md" || extension == "markdown" {
        let bytes = std::fs::read(path).map_err(|_| ConvertError::Reader(ReaderError::Io))?;
        (
            String::from_utf8_lossy(&bytes).replace("\r\n", "\n"),
            Vec::new(),
        )
    } else {
        let read = reader
            .read_other(path, staging, Duration::from_secs(3600), cancel)
            .map_err(ConvertError::Reader)?;
        let mut found = read.images;
        let mut markdown = read.markdown;
        if extension == "epub" || extension == "docx" {
            let (rewritten, from_package) = images_from_package(path, &markdown, &images_dir);
            markdown = rewritten;
            found.extend(from_package);
        }
        (markdown, found)
    };
    let (polished, report) = polish(
        &format!("<!-- page: 1 -->\n\n{}", body.trim()),
        &[],
        &[],
        &Default::default(),
    );
    // Imágenes del documento: solo las que existen y pasan las reglas de selección.
    let available: std::collections::HashSet<String> = found
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect();
    let missing: std::collections::HashSet<String> = images::all_referenced(&polished)
        .into_iter()
        .filter(|name| !available.contains(name))
        .collect();
    let with_images = images::drop_references(&polished, &missing);
    let rejected = images::rejected(&with_images, |name| images::facts(&images_dir.join(name)));
    let markdown = images::drop_references(&with_images, &rejected);
    let kept: Vec<PathBuf> = found
        .into_iter()
        .filter(|p| {
            let name = p.file_name().unwrap_or_default().to_string_lossy();
            markdown.contains(&format!("]({name})"))
        })
        .collect();
    let review = review::review(&markdown, &Default::default(), &PageStates::default());
    Ok(Converted {
        review,
        markdown,
        images: kept,
        report,
        scanned: false,
        doubtful_pages: Vec::new(),
        reread_pages: Vec::new(),
        figure_pages: Vec::new(),
        visual_available: false,
        seconds: started.elapsed().as_secs_f64(),
    })
}

/// Quita las marcas internas de página para exportar el Markdown final.
pub fn without_page_markers(markdown: &str) -> String {
    let kept: Vec<&str> = markdown
        .split("\n\n")
        .filter(|block| !block.trim_start().starts_with("<!-- page:"))
        .collect();
    kept.join("\n\n").trim().to_string() + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_formats_are_recognised_by_extension() {
        for name in [
            "a.pdf", "b.DOCX", "c.epub", "d.html", "e.htm", "f.txt", "g.md",
        ] {
            assert!(supported(Path::new(name)), "{name}");
        }
        for name in ["h.odt", "i.rtf", "j", "k.exe"] {
            assert!(!supported(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn images_are_taken_from_the_package_by_their_path() {
        use std::io::Write;
        let dir = std::env::temp_dir().join(format!("liblevo-paquete-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let package = dir.join("libro.epub");
        {
            let mut zip = zip::ZipWriter::new(std::fs::File::create(&package).unwrap());
            let options = zip::write::SimpleFileOptions::default();
            zip.start_file("OEBPS/images/fig.png", options).unwrap();
            zip.write_all(b"png-de-prueba").unwrap();
            zip.start_file("OEBPS/text/cap.xhtml", options).unwrap();
            zip.write_all(b"<html/>").unwrap();
            zip.finish().unwrap();
        }
        let markdown = "Texto.\n\n![Figura](../images/fig.png)\n\n![](no-existe.png)";
        let (out, files) = images_from_package(&package, markdown, &dir.join("images"));
        assert_eq!(files.len(), 1);
        assert_eq!(std::fs::read(&files[0]).unwrap(), b"png-de-prueba");
        assert_eq!(
            out,
            "Texto.\n\n![Figura](paq-0-fig.png)\n\n![](no-existe.png)"
        );
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn reread_budget_grows_with_the_document() {
        assert_eq!(reread_budget(1), Duration::from_secs(300));
        assert_eq!(reread_budget(16), Duration::from_secs(320));
        assert_eq!(reread_budget(88), Duration::from_secs(1760));
    }

    #[test]
    fn export_drops_only_page_markers() {
        let doc = "<!-- page: 1 -->\n\n# Título\n\nTexto\n\n<!-- page: 2 -->\n\nMás texto";
        assert_eq!(
            without_page_markers(doc),
            "# Título\n\nTexto\n\nMás texto\n"
        );
    }
}
