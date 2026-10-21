//! Herramienta de desarrollo para el EPUB ajeno: abre un EPUB y lo guarda, sin cambios o con
//! el texto de un bloque cambiado, para comprobar que el resto del paquete se conserva.
//!
//! `liblevo-epub ENTRADA.epub SALIDA.epub [CAPÍTULO BLOQUE TEXTO]`
//! Solo escribe recuentos; nunca contenido documental.

use liblevo_core::epub_package::EpubPackage;
use std::fs::File;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 2 && args.len() != 5 {
        eprintln!("Uso: liblevo-epub ENTRADA.epub SALIDA.epub [CAPÍTULO BLOQUE TEXTO]");
        return ExitCode::from(2);
    }
    let run = || -> Result<String, String> {
        let file = File::open(&args[0]).map_err(|e| e.to_string())?;
        let mut package = EpubPackage::open(file).map_err(|e| format!("{e:?}"))?;
        let chapters = package.chapters.len();
        let (mut blocks, mut editable) = (0, 0);
        for chapter in 0..chapters {
            let list = package.blocks(chapter);
            blocks += list.len();
            editable += list.iter().filter(|b| b.editable).count();
        }
        if args.len() == 5 {
            let chapter: usize = args[2].parse().map_err(|_| "capítulo no válido")?;
            let block: usize = args[3].parse().map_err(|_| "bloque no válido")?;
            package
                .set_text(chapter, block, &args[4])
                .map_err(|e| format!("{e:?}"))?;
        }
        let out = File::create(&args[1]).map_err(|e| e.to_string())?;
        package.save(out).map_err(|e| format!("{e:?}"))?;
        Ok(format!(
            "capítulos: {chapters}, bloques: {blocks}, editables: {editable}"
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
