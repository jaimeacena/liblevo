//! Herramienta de desarrollo: calcula la lista de revisión de un resultado ya convertido, sin
//! volver a convertir (para medir el revisor con errores plantados).
//!
//! `liblevo-revisar PDFIUM PDF DOCUMENTO.md` escribe la lista en JSON por la salida estándar.

use liblevo_worker::pdf_styles::page_texts;
use liblevo_worker::review::{PageStates, review};
use pdfium_render::prelude::*;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        eprintln!("Uso: liblevo-revisar PDFIUM PDF DOCUMENTO.md");
        return ExitCode::from(2);
    }
    let run = || -> Result<String, String> {
        let pdfium =
            Pdfium::new(Pdfium::bind_to_library(&args[0]).map_err(|e| format!("PDFium: {e:?}"))?);
        let markdown = std::fs::read_to_string(&args[2]).map_err(|e| e.to_string())?;
        let items = review(
            &markdown.replace("\r\n", "\n"),
            &page_texts(&pdfium, Path::new(&args[1])),
            &PageStates::default(),
        );
        serde_json::to_string(&items).map_err(|e| e.to_string())
    };
    match run() {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
