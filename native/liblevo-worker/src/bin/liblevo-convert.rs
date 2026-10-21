//! Herramienta de desarrollo: convierte un PDF con el mismo camino que la app y escribe
//! `document.md`, `document.epub`, `images/` y `run.json` en la carpeta de salida, para medirlo con `liblevo-eval`.
//!
//! `liblevo-convert PDFIUM CARPETA_XBERG CARPETA_MODELOS PDF SALIDA`
//! Solo escribe recuentos en la salida estándar, nunca contenido documental.

use liblevo_core::markdown_epub::{BookMetadata, ImageFile, write_markdown_epub};
use liblevo_worker::convert::{Visual, convert_other, convert_pdf, without_page_markers};
use liblevo_worker::visual_reader::VisualReader;
use liblevo_worker::xberg_reader::XbergReader;
use pdfium_render::prelude::*;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        eprintln!("Uso: liblevo-convert PDFIUM CARPETA_XBERG CARPETA_MODELOS PDF SALIDA");
        return ExitCode::from(2);
    }
    let out = PathBuf::from(&args[4]);
    let run = || -> Result<String, String> {
        std::fs::create_dir_all(&out).map_err(|e| e.to_string())?;
        let pdfium =
            Pdfium::new(Pdfium::bind_to_library(&args[0]).map_err(|e| format!("PDFium: {e:?}"))?);
        let reader = XbergReader {
            program_dir: PathBuf::from(&args[1]),
            models_dir: PathBuf::from(&args[2]),
        };
        let staging = out.join("staging");
        // Herramienta de medición: siempre desde cero, sin puntos de control anteriores.
        let checkpoints = out.join("checkpoint");
        let _ = std::fs::remove_dir_all(&checkpoints);
        // Modelo visual opcional para las páginas dudosas de los escaneos.
        let visual = match (
            std::env::var_os("LIBLEVO_OVIS"),
            std::env::var_os("LIBLEVO_OVIS_MODEL"),
        ) {
            (Some(program), Some(model)) => Some(VisualReader {
                program_dir: PathBuf::from(program),
                model_dir: PathBuf::from(model),
            }),
            _ => None,
        };
        let source = Path::new(&args[3]);
        let is_pdf = source
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("pdf"));
        let converted = if is_pdf {
            convert_pdf(
                &pdfium,
                &reader,
                source,
                &staging,
                &checkpoints,
                Visual {
                    reader: visual.as_ref(),
                    // Medición del modo de lectura visual completa.
                    full: std::env::var_os("LIBLEVO_VISUAL_COMPLETA").is_some(),
                },
                &AtomicBool::new(false),
            )
        } else {
            convert_other(&reader, source, &staging, &AtomicBool::new(false))
        }
        .map_err(|e| e.message())?;
        std::fs::write(out.join("document.md"), &converted.markdown).map_err(|e| e.to_string())?;
        let images = out.join("images");
        std::fs::create_dir_all(&images).map_err(|e| e.to_string())?;
        for image in &converted.images {
            if let Some(name) = image.file_name() {
                std::fs::copy(image, images.join(name)).map_err(|e| e.to_string())?;
            }
        }
        // EPUB del mismo resultado, sin las marcas internas de página.
        let epub_images: Vec<ImageFile> = converted
            .images
            .iter()
            .filter_map(|path| {
                Some(ImageFile {
                    name: path.file_name()?.to_string_lossy().into_owned(),
                    bytes: std::fs::read(path).ok()?,
                })
            })
            .collect();
        let pdf = Path::new(&args[3]);
        let metadata = BookMetadata {
            title: pdf
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            author: None,
            language: "und".into(),
            identifier: format!(
                "urn:sha256:{}",
                liblevo_worker::xberg_reader::sha256_file(pdf).map_err(|e| e.message())?
            ),
            modified: "2026-10-19T00:00:00Z".into(),
            cover: None,
        };
        let file = std::fs::File::create(out.join("document.epub")).map_err(|e| e.to_string())?;
        write_markdown_epub(
            &without_page_markers(&converted.markdown),
            &metadata,
            &epub_images,
            file,
        )
        .map_err(|e| format!("EPUB: {e}"))?;
        let _ = std::fs::remove_dir_all(&staging);
        let _ = std::fs::remove_dir_all(&checkpoints);
        std::fs::write(
            out.join("review.json"),
            serde_json::to_string(&converted.review).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        let run = serde_json::json!({
            "status": "ok",
            "seconds": (converted.seconds * 100.0).round() / 100.0,
            "scanned": converted.scanned,
            "doubtful_pages": converted.doubtful_pages,
            "figure_pages": converted.figure_pages,
            "reread_pages": converted.reread_pages,
            "visual_available": converted.visual_available,
        });
        std::fs::write(out.join("run.json"), run.to_string()).map_err(|e| e.to_string())?;
        Ok(format!(
            "escaneo: {}, márgenes quitados: {}, rayas repuestas: {}, palabras unidas: {}, títulos: {}, imágenes: {}",
            if converted.scanned { "sí" } else { "no" },
            converted.report.removed_margins,
            converted.report.restored_dashes,
            converted.report.joined_words,
            converted.report.headings,
            converted.images.len()
        ))
    };
    match run() {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            let _ = std::fs::write(
                out.join("run.json"),
                serde_json::json!({"status": "failed", "note": message}).to_string(),
            );
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
