//! Herramienta de desarrollo: aplica el pulido (cabeceras, guiones y títulos) a la salida de un
//! lector para medirla con `liblevo-eval`.
//!
//! `liblevo-posproceso PDFIUM PDF ENTRADA.md SALIDA.md [PRIMERA_PÁGINA]`
//! Solo escribe recuentos en la salida estándar, nunca contenido documental.

use liblevo_worker::pdf_styles::{outline, page_texts, styled_lines};
use liblevo_worker::polish::polish;
use pdfium_render::prelude::*;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        eprintln!("Uso: liblevo-posproceso PDFIUM PDF ENTRADA.md SALIDA.md [PRIMERA_PÁGINA]");
        return ExitCode::from(2);
    }
    let first_page = args.get(4).and_then(|v| v.parse().ok()).unwrap_or(1);
    let run = || -> Result<String, String> {
        let pdfium =
            Pdfium::new(Pdfium::bind_to_library(&args[0]).map_err(|e| format!("PDFium: {e:?}"))?);
        let started = std::time::Instant::now();
        let lines = styled_lines(&pdfium, Path::new(&args[1]), first_page)?;
        let styles_seconds = started.elapsed().as_secs_f64();
        let markdown = std::fs::read_to_string(&args[2]).map_err(|e| e.to_string())?;
        let started = std::time::Instant::now();
        let marks = outline(&pdfium, Path::new(&args[1]));
        let texts = page_texts(&pdfium, Path::new(&args[1]));
        let (result, report) = polish(&markdown, &lines, &marks, &texts);
        let polish_seconds = started.elapsed().as_secs_f64();
        std::fs::write(&args[3], result).map_err(|e| e.to_string())?;
        Ok(format!(
            "marcadores: {}, márgenes quitados: {}, rayas repuestas: {}, palabras unidas: {}, títulos propuestos: {}, estilos: {styles_seconds:.1} s, pulido: {polish_seconds:.1} s",
            marks.len(),
            report.removed_margins,
            report.restored_dashes,
            report.joined_words,
            report.headings
        ))
    };
    match run() {
        Ok(summary) => {
            println!("{summary}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
