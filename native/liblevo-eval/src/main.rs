//! `liblevo-eval`: herramienta de desarrollo para medir conversiones con el conjunto de evaluación.
//!
//! - `verify --manifest M --pdf-dir D`: comprueba que los PDF existen y coinciden con su SHA-256.
//! - `measure --manifest M --pdf-dir D --pdfium DLL --references R --candidate C --out F`: mide la
//!   salida de un candidato (`C/<id>/document.md` y `C/<id>/run.json`) y escribe JSON.
//! - `table F...`: tabla Markdown que compara varios resultados.
//!
//! No forma parte de la app ni del instalador y no tiene código de red.

mod manifest;
mod markdown;
mod measure;
mod reference;
mod text;

use measure::DocResult;
use pdfium_render::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[derive(Serialize, Deserialize)]
struct Results {
    candidate: String,
    documents: Vec<DocResult>,
}

fn arguments(args: &[String]) -> Result<BTreeMap<String, Vec<String>>, String> {
    let mut map: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut key = String::from("_");
    for arg in args {
        if let Some(name) = arg.strip_prefix("--") {
            key = name.to_string();
            map.entry(key.clone()).or_default();
        } else {
            map.entry(key.clone()).or_default().push(arg.clone());
        }
    }
    Ok(map)
}

fn one(map: &BTreeMap<String, Vec<String>>, name: &str) -> Result<PathBuf, String> {
    map.get(name)
        .and_then(|v| v.first())
        .map(PathBuf::from)
        .ok_or_else(|| format!("Falta --{name}"))
}

fn verify(map: &BTreeMap<String, Vec<String>>) -> Result<(), String> {
    let manifest = manifest::load(&one(map, "manifest")?)?;
    let dir = one(map, "pdf-dir")?;
    let mut failures = 0;
    for document in &manifest.documents {
        let path = dir.join(document.file_name());
        match manifest::sha256_file(&path) {
            Ok(hash) if hash == document.sha256 => println!("ok      {}", document.id),
            Ok(_) => {
                failures += 1;
                println!("DISTINTO {}", document.id);
            }
            Err(_) => {
                failures += 1;
                println!("FALTA   {}", document.id);
            }
        }
    }
    if failures > 0 {
        return Err(format!(
            "{failures} documentos no coinciden con el manifiesto"
        ));
    }
    Ok(())
}

fn measure_all(map: &BTreeMap<String, Vec<String>>) -> Result<(), String> {
    let manifest = manifest::load(&one(map, "manifest")?)?;
    let pdf_dir = one(map, "pdf-dir")?;
    let references = one(map, "references")?;
    let candidate = one(map, "candidate")?;
    let out = one(map, "out")?;
    let pdfium_path = one(map, "pdfium")?;
    let only: Vec<String> = map.get("only").cloned().unwrap_or_default();
    let pdfium = Pdfium::new(
        Pdfium::bind_to_library(&pdfium_path)
            .map_err(|e| format!("No se pudo cargar PDFium: {e:?}"))?,
    );
    let name = candidate
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut documents = Vec::new();
    for document in &manifest.documents {
        if !only.is_empty() && !only.contains(&document.id) {
            continue;
        }
        let path = pdf_dir.join(document.file_name());
        let reference = reference::read(&pdfium, &path, |count| document.page_range(count));
        let reference = match reference {
            Ok(r) => Some(r),
            Err(e) => {
                eprintln!("{}: {e}", document.id);
                None
            }
        };
        let result =
            measure::measure_document(document, reference.as_ref(), &references, &candidate);
        eprintln!("{:40} {}", document.id, result.status);
        documents.push(result);
    }
    let results = Results {
        candidate: name,
        documents,
    };
    std::fs::write(
        &out,
        serde_json::to_string_pretty(&results).map_err(|e| e.to_string())?,
    )
    .map_err(|e| format!("No se pudo escribir {}: {e}", out.display()))?;
    Ok(())
}

fn pct(value: Option<f64>) -> String {
    value.map_or("—".into(), |v| format!("{:.1}", v * 100.0))
}

fn table(map: &BTreeMap<String, Vec<String>>) -> Result<(), String> {
    let files = map.get("_").cloned().unwrap_or_default();
    let mut all = Vec::new();
    for file in files.iter().skip(1) {
        let raw = std::fs::read_to_string(Path::new(file)).map_err(|e| format!("{file}: {e}"))?;
        all.push(serde_json::from_str::<Results>(&raw).map_err(|e| format!("{file}: {e}"))?);
    }
    println!(
        "| Documento | Candidato | Estado | Palabras % | Precisión % | Cifras % | Orden % | Error car. % | Fugas margen | Marcadores % | Títulos | Tablas | Imágenes | s/página |"
    );
    println!("|---|---|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|");
    let mut ids: Vec<String> = Vec::new();
    for results in &all {
        for document in &results.documents {
            if !ids.contains(&document.id) {
                ids.push(document.id.clone());
            }
        }
    }
    ids.sort();
    for id in ids {
        for results in &all {
            let Some(d) = results.documents.iter().find(|d| d.id == id) else {
                continue;
            };
            let leaks = match (d.margin_leaks, d.margin_source) {
                (Some(l), Some(s)) => format!("{l}/{s}"),
                _ => "—".into(),
            };
            let per_page = match (d.seconds, d.pages_measured) {
                (Some(s), p) if p > 0 => format!("{:.1}", s / p as f64),
                _ => "—".into(),
            };
            println!(
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |",
                d.id,
                results.candidate,
                d.status,
                pct(d.words_recall),
                pct(d.words_precision),
                pct(d.numbers_recall),
                pct(d.order),
                pct(d.cer),
                leaks,
                pct(d.outline_recall),
                d.headings,
                d.tables,
                d.images,
                per_page
            );
        }
    }
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some(command) = args.first().cloned() else {
        eprintln!("Uso: liblevo-eval verify|measure|table ...");
        return ExitCode::from(2);
    };
    let outcome = arguments(&args).and_then(|map| match command.as_str() {
        "verify" => verify(&map),
        "measure" => measure_all(&map),
        "table" => table(&map),
        other => Err(format!("Orden desconocida: {other}")),
    });
    match outcome {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
