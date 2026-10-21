//! Herramienta de desarrollo: traduce un resultado ya convertido (Markdown con marcas de página).
//!
//! `liblevo-traducir PROGRAMAS MODELO.gguf DOCUMENTO.md IDIOMA SALIDA.md [GLOSARIO.tsv]`
//!
//! `IDIOMA` es un código (`es`, `en`, `fr`, `de`, `it`, `pt`). El glosario tiene una línea por
//! término: original, tabulador, traducción. Los bloques traducidos se guardan junto a la salida
//! (`SALIDA.md.bloques`) para reanudar, y el informe en `SALIDA.json`. `LIBLEVO_MAX_BLOQUES`
//! limita la traducción a los primeros bloques de texto (para medir). Si `DOCUMENTO` es un EPUB,
//! se traduce en su sitio sobre el propio paquete y `SALIDA` es el EPUB traducido.

use liblevo_core::epub_package::EpubPackage;
use liblevo_worker::translate::{
    Term, TranslationReport, Translator, language_name, translate_epub, translate_markdown,
};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::atomic::AtomicBool;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if !(5..=6).contains(&args.len()) {
        eprintln!(
            "Uso: liblevo-traducir PROGRAMAS MODELO.gguf DOCUMENTO.md IDIOMA SALIDA.md [GLOSARIO.tsv]"
        );
        return ExitCode::from(2);
    }
    let run = || -> Result<(), String> {
        let translator = Translator::new(PathBuf::from(&args[0]), PathBuf::from(&args[1]));
        translator.verify().map_err(|e| e.message())?;
        let language = language_name(&args[3]).ok_or("Idioma no admitido.")?;
        let glossary: Vec<Term> = match args.get(5) {
            Some(path) => std::fs::read_to_string(path)
                .map_err(|e| e.to_string())?
                .lines()
                .filter_map(|line| line.split_once('\t'))
                .map(|(a, b)| (a.trim().to_string(), b.trim().to_string()))
                .filter(|(a, b)| !a.is_empty() && !b.is_empty())
                .collect(),
            None => Vec::new(),
        };
        let output = Path::new(&args[4]);
        let cache = PathBuf::from(format!("{}.bloques", output.display()));
        let work = std::env::temp_dir().join(format!("liblevo-traducir-{}", std::process::id()));
        let cancel = AtomicBool::new(false);
        let mut run = |text: &str, terms: &[Term]| {
            translator.translate(text, language, terms, &work, &cancel)
        };
        let mut progress = |done, total| eprintln!("{done}/{total}");
        let report: TranslationReport = if args[2].to_lowercase().ends_with(".epub") {
            let file = std::fs::File::open(&args[2]).map_err(|e| e.to_string())?;
            let mut package = EpubPackage::open(file).map_err(|_| "EPUB sin estructura.")?;
            let report = translate_epub(
                &mut package,
                language,
                &args[3],
                translator.model_hash(),
                &glossary,
                &cache,
                &mut run,
                &mut progress,
            )
            .map_err(|e| e.message())?;
            let out = std::fs::File::create(output).map_err(|e| e.to_string())?;
            package
                .save(out)
                .map_err(|_| "No se pudo guardar el EPUB.")?;
            report
        } else {
            let mut markdown = std::fs::read_to_string(&args[2]).map_err(|e| e.to_string())?;
            if let Some(limit) = std::env::var("LIBLEVO_MAX_BLOQUES")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
            {
                markdown = first_text_blocks(&markdown, limit);
            }
            let (translated, report) = translate_markdown(
                &markdown,
                language,
                translator.model_hash(),
                &glossary,
                &cache,
                &mut run,
                &mut progress,
            )
            .map_err(|e| e.message())?;
            std::fs::write(output, translated).map_err(|e| e.to_string())?;
            report
        };
        let _ = std::fs::remove_dir_all(&work);
        std::fs::write(
            output.with_extension("json"),
            serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
        println!(
            "{} bloques ({} traducidos, {} reutilizados), {} dudas, {:.0} s",
            report.blocks,
            report.translated,
            report.reused,
            report.doubts.len(),
            report.seconds
        );
        Ok(())
    };
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// Recorta el Markdown tras `limit` bloques de texto (las marcas de página y las imágenes no
/// cuentan).
fn first_text_blocks(markdown: &str, limit: usize) -> String {
    let mut out = Vec::new();
    let mut text = 0;
    for block in markdown.replace("\r\n", "\n").split("\n\n") {
        let trimmed = block.trim();
        if !(trimmed.starts_with("<!--") || trimmed.starts_with("![") || trimmed.is_empty()) {
            if text == limit {
                break;
            }
            text += 1;
        }
        out.push(block.to_string());
    }
    out.join("\n\n")
}
