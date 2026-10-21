//! Conversión en segundo plano, resultados guardados y exportación.
//!
//! La lectura pesada corre en el proceso aparte de Xberg; aquí solo se coordina. Cada resultado se
//! escribe primero en una carpeta temporal y se sustituye de forma atómica, de modo que un cierre a
//! mitad nunca deja un resultado a medias. El original no se modifica nunca.

use liblevo_core::epub_package::EpubPackage;
use liblevo_core::markdown_epub::{BookMetadata, ImageFile, math_event, write_markdown_epub};
use liblevo_worker::convert::{Visual, convert_other, convert_pdf, without_page_markers};
use liblevo_worker::review::{self, PageStates, Reason, ReviewItem, block_key, is_content_block};
use liblevo_worker::translate::{
    LANGUAGES, MODEL_1_8B, MODEL_7B, Term, TranslationReport, Translator, translate_epub,
    translate_markdown,
};
use liblevo_worker::visual_reader::VisualReader;
use liblevo_worker::xberg_reader::{ReaderError, XbergReader};
use pulldown_cmark::{CowStr, Event, Options, Parser, Tag, html};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Carpeta donde el catálogo instala los componentes descargados.
pub fn components_dir(data: &Path) -> PathBuf {
    data.join("components")
}

/// Busca una carpeta de componente: variable de entorno (desarrollo), carpeta de componentes
/// descargados y carpeta `components` junto al ejecutable, en ese orden.
fn component_folder(data: &Path, variable: &str, folder: &str) -> Option<PathBuf> {
    let beside = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|p| p.join("components")));
    std::env::var_os(variable)
        .map(PathBuf::from)
        .into_iter()
        .chain([components_dir(data).join(folder)])
        .chain(beside.map(|b| b.join(folder)))
        .find(|p| p.is_dir())
}

const MISSING_READER: &str =
    "Falta el componente «Lectura de PDF». Descárgalo desde Componentes y vuelve a convertir.";

/// Componentes de lectura (Xberg y sus modelos).
pub fn reader(data: &Path) -> Result<XbergReader, String> {
    Ok(XbergReader {
        program_dir: component_folder(data, "LIBLEVO_XBERG", "xberg").ok_or(MISSING_READER)?,
        models_dir: component_folder(data, "LIBLEVO_XBERG_MODELS", "xberg-modelos")
            .ok_or(MISSING_READER)?,
    })
}

/// Modelo visual para releer las páginas dudosas de los escaneos. Es opcional: sin él, las páginas
/// dudosas conservan su OCR y la app lo indica.
pub fn visual_reader(data: &Path) -> Option<VisualReader> {
    Some(VisualReader {
        program_dir: component_folder(data, "LIBLEVO_OVIS", "ovis")?,
        model_dir: component_folder(data, "LIBLEVO_OVIS_MODEL", "ovis-modelo")?,
    })
}

#[derive(Default)]
pub struct Conversions {
    running: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

impl Conversions {
    pub fn start(&self, id: &str) -> Option<Arc<AtomicBool>> {
        let mut running = self.running.lock().ok()?;
        if running.contains_key(id) {
            return None;
        }
        let cancel = Arc::new(AtomicBool::new(false));
        running.insert(id.to_string(), cancel.clone());
        Some(cancel)
    }

    pub fn finish(&self, id: &str) {
        if let Ok(mut running) = self.running.lock() {
            running.remove(id);
        }
    }

    pub fn cancel(&self, id: &str) -> bool {
        self.running
            .lock()
            .ok()
            .and_then(|running| running.get(id).cloned())
            .is_some_and(|flag| {
                flag.store(true, Ordering::Relaxed);
                true
            })
    }

    pub fn is_running(&self, id: &str) -> bool {
        self.running
            .lock()
            .map(|running| running.contains_key(id))
            .unwrap_or(false)
    }
}

/// ¿Es un PDF? (los demás formatos no tienen páginas que dibujar).
pub fn is_pdf(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// Texto (Markdown) de una página del resultado, para el editor mínimo.
pub fn page_markdown(data: &Path, id: &str, page: u32) -> Result<String, String> {
    let markdown = fs::read_to_string(result_dir(data, id).join("document.md"))
        .map_err(|_| "Este documento todavía no tiene resultado.")?;
    Ok(liblevo_worker::doubt::markdown_pages(&markdown)
        .remove(&page)
        .unwrap_or_default()
        .trim()
        .to_string())
}

/// Guarda el texto editado de una página del resultado (escritura atómica). No puede crear ni
/// borrar marcas de página.
pub fn save_page(data: &Path, id: &str, page: u32, text: &str) -> Result<(), String> {
    if text.contains("<!-- page:") {
        return Err("El texto no puede contener marcas de página.".into());
    }
    let path = result_dir(data, id).join("document.md");
    let markdown =
        fs::read_to_string(&path).map_err(|_| "Este documento todavía no tiene resultado.")?;
    if !markdown.contains(&format!("<!-- page: {page} -->")) {
        return Err("Esa página no está en el resultado.".into());
    }
    let updated =
        liblevo_worker::visual_reader::replace_page(&markdown, page, &text.replace("\r\n", "\n"));
    write_atomic(&path, updated.as_bytes())
}

/// Busca un editor de EPUB instalado: Sigil o, si no, el editor de Calibre.
fn epub_editor() -> Option<(PathBuf, &'static str)> {
    let roots: Vec<PathBuf> = ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
        .iter()
        .filter_map(std::env::var_os)
        .map(PathBuf::from)
        .collect();
    let candidates = [
        ("Sigil/Sigil.exe", "Sigil"),
        ("Programs/Sigil/Sigil.exe", "Sigil"),
        ("Calibre2/ebook-edit.exe", "Calibre"),
        ("Calibre/ebook-edit.exe", "Calibre"),
    ];
    candidates.iter().find_map(|(relative, name)| {
        roots
            .iter()
            .map(|root| root.join(relative))
            .find(|path| path.is_file())
            .map(|path| (path, *name))
    })
}

/// Abre un EPUB guardado en el editor instalado (solo un archivo `.epub` que exista).
pub fn open_in_epub_editor(epub: &Path) -> Result<String, String> {
    let is_epub = epub
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("epub"));
    if !is_epub || !epub.is_file() {
        return Err("No se encuentra el EPUB guardado.".into());
    }
    let (program, name) =
        epub_editor().ok_or("No se encontró Sigil ni el editor de Calibre en este equipo.")?;
    std::process::Command::new(program)
        .arg(epub)
        .spawn()
        .map_err(|_| format!("No se pudo abrir {name}."))?;
    Ok(name.to_string())
}

/// Copia de trabajo de un EPUB ajeno editado (la original nunca se toca).
fn epub_working_copy(data: &Path, id: &str) -> PathBuf {
    data.join("epub").join(format!("{id}.epub"))
}

/// Un bloque de texto de un capítulo del EPUB original.
#[derive(Serialize)]
pub struct EpubBlockView {
    pub index: usize,
    pub tag: String,
    pub text: String,
    pub editable: bool,
}

#[derive(Serialize)]
pub struct EpubChapterView {
    pub chapters: usize,
    pub chapter: usize,
    pub blocks: Vec<EpubBlockView>,
    /// Hay ediciones guardadas en la copia de trabajo.
    pub edited: bool,
}

fn open_epub(data: &Path, id: &str, original: &Path) -> Result<EpubPackage<fs::File>, String> {
    let working = epub_working_copy(data, id);
    let source = if working.is_file() {
        &working
    } else {
        original
    };
    let file = fs::File::open(source).map_err(|_| "No se encuentra el EPUB.")?;
    EpubPackage::open(file)
        .map_err(|_| "Este EPUB no se puede abrir (le falta su estructura).".into())
}

/// Bloques de un capítulo del EPUB original (o de su copia de trabajo, si ya se editó).
pub fn epub_chapter(
    data: &Path,
    id: &str,
    original: &Path,
    chapter: usize,
) -> Result<EpubChapterView, String> {
    let mut package = open_epub(data, id, original)?;
    let chapters = package.chapters.len();
    let chapter = chapter.min(chapters.saturating_sub(1));
    Ok(EpubChapterView {
        chapters,
        chapter,
        blocks: package
            .blocks(chapter)
            .into_iter()
            .map(|b| EpubBlockView {
                index: b.index,
                tag: b.tag,
                text: b.text,
                editable: b.editable,
            })
            .collect(),
        edited: epub_working_copy(data, id).is_file(),
    })
}

/// Cambia el texto de un bloque editable y guarda la copia de trabajo de forma atómica: lo demás
/// del paquete se copia tal cual.
pub fn epub_set_text(
    data: &Path,
    id: &str,
    original: &Path,
    chapter: usize,
    index: usize,
    text: &str,
) -> Result<(), String> {
    let mut package = open_epub(data, id, original)?;
    package
        .set_text(chapter, index, text.trim())
        .map_err(|_| "Ese bloque tiene formato interno: edítalo en Sigil o Calibre.")?;
    let working = epub_working_copy(data, id);
    fs::create_dir_all(working.parent().unwrap_or(data))
        .map_err(|_| "No se pudo guardar la edición.")?;
    let temporary = working.with_extension("liblevo-tmp");
    let file = fs::File::create(&temporary).map_err(|_| "No se pudo guardar la edición.")?;
    let saved = package.save(file).map(|_| ());
    drop(package);
    if saved.is_err() || fs::rename(&temporary, &working).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err("No se pudo guardar la edición.".into());
    }
    Ok(())
}

/// Guarda el EPUB editado (o el original, si no hay ediciones) en `target`.
pub fn epub_save_as(data: &Path, id: &str, original: &Path, target: &Path) -> Result<(), String> {
    let working = epub_working_copy(data, id);
    let source = if working.is_file() {
        working
    } else {
        original.to_path_buf()
    };
    let bytes = fs::read(source).map_err(|_| "No se encuentra el EPUB.")?;
    write_atomic(target, &bytes)
}

/// Descarta las ediciones del EPUB (borra la copia de trabajo).
pub fn epub_discard(data: &Path, id: &str) {
    let _ = fs::remove_file(epub_working_copy(data, id));
}

/// Traductor instalado: el modelo rápido (1,8B) o el de más calidad (7B), con su programa.
pub fn translator(data: &Path, quality: bool) -> Option<Translator> {
    let program_dir = component_folder(data, "LIBLEVO_TRADUCCION", "traduccion")?;
    let model = if quality {
        component_folder(data, "LIBLEVO_TRADUCCION_CALIDAD", "traduccion-calidad")?.join(MODEL_7B.0)
    } else {
        component_folder(data, "LIBLEVO_TRADUCCION_RAPIDA", "traduccion-rapida")?.join(MODEL_1_8B.0)
    };
    model.is_file().then(|| Translator::new(program_dir, model))
}

/// Carpeta de la traducción de un resultado a un idioma. Va dentro del resultado: si se vuelve a
/// convertir, la traducción anterior desaparece con él, porque ya no le corresponde.
pub fn translation_dir(data: &Path, id: &str, language: &str) -> PathBuf {
    result_dir(data, id).join("traducciones").join(language)
}

/// Markdown que se muestra o exporta: el resultado o una de sus traducciones.
fn document_path(data: &Path, id: &str, translation: Option<&str>) -> PathBuf {
    match translation {
        Some(language) => translation_dir(data, id, language).join("document.md"),
        None => result_dir(data, id).join("document.md"),
    }
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TranslationSummary {
    pub language: String,
    pub label: String,
    pub quality: bool,
    pub blocks: usize,
    pub doubts: usize,
    pub seconds: f64,
    /// EPUB original: bloques con formato interno que quedan sin traducir.
    #[serde(default)]
    pub untouched: usize,
}

/// Traducciones guardadas de un documento.
pub fn translations(data: &Path, id: &str) -> Vec<TranslationSummary> {
    let mut found: Vec<TranslationSummary> = LANGUAGES
        .iter()
        .filter_map(|(code, _, _)| {
            let folder = translation_dir(data, id, code);
            if !folder.join("document.md").is_file() {
                return None;
            }
            serde_json::from_slice(&fs::read(folder.join("summary.json")).ok()?).ok()
        })
        .collect();
    found.sort_by(|a, b| a.label.cmp(&b.label));
    found
}

/// Glosario escrito por el usuario: una línea por término, «original = traducción».
pub fn parse_glossary(text: &str) -> Result<Vec<Term>, String> {
    let mut terms = Vec::new();
    for line in text.lines().map(str::trim).filter(|l| !l.is_empty()) {
        let Some((source, target)) = line.split_once('=') else {
            return Err(format!(
                "Cada línea del glosario debe ser «original = traducción»: «{line}»."
            ));
        };
        let (source, target) = (source.trim(), target.trim());
        if source.is_empty() || target.is_empty() {
            return Err(format!("Falta un lado en la línea del glosario «{line}»."));
        }
        terms.push((source.to_string(), target.to_string()));
    }
    if terms.len() > 200 {
        return Err("El glosario admite hasta 200 términos.".into());
    }
    Ok(terms)
}

/// Idioma admitido: (nombre para el modelo, nombre en la interfaz).
fn language_names(language: &str) -> Result<(&'static str, &'static str), String> {
    LANGUAGES
        .iter()
        .find(|(code, _, _)| *code == language)
        .map(|(_, name, label)| (*name, *label))
        .ok_or_else(|| "Idioma no admitido.".to_string())
}

/// Traductor instalado y verificado.
fn ready_translator(data: &Path, quality: bool) -> Result<Translator, String> {
    let translator = translator(data, quality).ok_or(if quality {
        "Falta el componente «Traducción de más calidad». Descárgalo desde Componentes."
    } else {
        "Falta el componente «Traducción». Descárgalo desde Componentes."
    })?;
    translator.verify().map_err(|_| {
        "El componente de traducción no está verificado. Vuelve a descargarlo desde Componentes."
    })?;
    Ok(translator)
}

fn translation_error(error: ReaderError) -> String {
    match error {
        ReaderError::Cancelled => "Traducción cancelada.".to_string(),
        other => other.message(),
    }
}

/// Bloques ya traducidos del EPUB original, para reanudar (se borran al quitar el documento).
pub fn epub_translation_cache(data: &Path, id: &str) -> PathBuf {
    data.join("epub").join(format!("{id}-traduccion"))
}

/// Traduce el EPUB original en su copia de trabajo, la misma que muestra «Editar EPUB original»:
/// se conserva el paquete entero y solo cambia el texto de los bloques sin formato interno.
#[allow(clippy::too_many_arguments)]
pub fn translate_epub_original(
    data: &Path,
    id: &str,
    original: &Path,
    language: &str,
    quality: bool,
    glossary: &[Term],
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<TranslationSummary, String> {
    let (name, label) = language_names(language)?;
    let translator = ready_translator(data, quality)?;
    let mut package = open_epub(data, id, original)?;
    let work = work_dir(data, id).join("traduccion");
    let outcome = translate_epub(
        &mut package,
        name,
        language,
        translator.model_hash(),
        glossary,
        &epub_translation_cache(data, id).join(language),
        &mut |text, terms| translator.translate(text, name, terms, &work, cancel),
        progress,
    );
    let _ = fs::remove_dir_all(&work);
    let report = outcome.map_err(translation_error)?;
    let working = epub_working_copy(data, id);
    fs::create_dir_all(working.parent().unwrap_or(data))
        .map_err(|_| "No se pudo guardar la traducción.")?;
    let temporary = working.with_extension("liblevo-tmp");
    let file = fs::File::create(&temporary).map_err(|_| "No se pudo guardar la traducción.")?;
    let saved = package.save(file).map(|_| ());
    drop(package);
    if saved.is_err() || fs::rename(&temporary, &working).is_err() {
        let _ = fs::remove_file(&temporary);
        return Err("No se pudo guardar la traducción.".into());
    }
    Ok(TranslationSummary {
        language: language.to_string(),
        label: label.to_string(),
        quality,
        blocks: report.blocks,
        doubts: report.doubts.len(),
        seconds: (report.seconds * 10.0).round() / 10.0,
        untouched: report.untouched,
    })
}

/// Traduce el resultado guardado y publica la traducción de forma atómica. Los bloques ya
/// traducidos se guardan uno a uno, de modo que una cancelación o un cierre se reanudan.
pub fn translate(
    data: &Path,
    id: &str,
    language: &str,
    quality: bool,
    glossary: &[Term],
    cancel: &AtomicBool,
    progress: &mut dyn FnMut(usize, usize),
) -> Result<TranslationSummary, String> {
    let (name, label) = language_names(language)?;
    let markdown = fs::read_to_string(result_dir(data, id).join("document.md"))
        .map_err(|_| "Este documento todavía no tiene resultado.")?;
    let translator = ready_translator(data, quality)?;
    let folder = translation_dir(data, id, language);
    let work = work_dir(data, id).join("traduccion");
    let outcome = translate_markdown(
        &markdown,
        name,
        translator.model_hash(),
        glossary,
        &folder.join("bloques"),
        &mut |text, terms| translator.translate(text, name, terms, &work, cancel),
        progress,
    );
    let _ = fs::remove_dir_all(&work);
    let (translated, report) = outcome.map_err(translation_error)?;
    let summary = TranslationSummary {
        language: language.to_string(),
        label: label.to_string(),
        quality,
        blocks: report.blocks,
        doubts: report.doubts.len(),
        seconds: (report.seconds * 10.0).round() / 10.0,
        untouched: 0,
    };
    write_atomic(
        &folder.join("report.json"),
        serde_json::to_string(&report)
            .unwrap_or_default()
            .as_bytes(),
    )?;
    write_atomic(
        &folder.join("summary.json"),
        serde_json::to_string(&summary)
            .unwrap_or_default()
            .as_bytes(),
    )?;
    write_atomic(&folder.join("document.md"), translated.as_bytes())?;
    Ok(summary)
}

/// Un aviso de la traducción para la revisión.
#[derive(Serialize)]
pub struct TranslationDoubtView {
    pub page: u32,
    pub block: usize,
    pub reason: String,
    pub excerpt: String,
}

/// Avisos de una traducción, con un extracto del bloque traducido.
pub fn translation_doubts(
    data: &Path,
    id: &str,
    language: &str,
) -> Result<Vec<TranslationDoubtView>, String> {
    let folder = translation_dir(data, id, language);
    let report: TranslationReport = fs::read(folder.join("report.json"))
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .ok_or("Esta traducción no tiene informe.")?;
    let markdown = fs::read_to_string(folder.join("document.md")).unwrap_or_default();
    Ok(report
        .doubts
        .into_iter()
        .map(|(page, block, doubt)| {
            let excerpt = locate_block(&markdown, page, block)
                .map(|(start, end)| {
                    let text: String = markdown[start..end].chars().take(160).collect();
                    text.replace('\n', " ")
                })
                .unwrap_or_default();
            TranslationDoubtView {
                page,
                block,
                reason: doubt.reason().to_string(),
                excerpt,
            }
        })
        .collect())
}

/// Carpeta del resultado guardado de un documento.
pub fn result_dir(data: &Path, id: &str) -> PathBuf {
    data.join("results").join(id)
}

/// Carpeta de trabajo de un documento; dentro, `checkpoint` sobrevive a un cierre.
pub fn work_dir(data: &Path, id: &str) -> PathBuf {
    data.join("work").join(id)
}

/// Hay una conversión interrumpida que se puede reanudar.
pub fn resumable(data: &Path, id: &str) -> bool {
    liblevo_worker::checkpoint::exists(&work_dir(data, id).join("checkpoint"))
}

/// Al abrir la app: deja cada carpeta de trabajo como tras un cierre limpio. Si un cierre llegó
/// entre los dos cambios de nombre de la publicación, devuelve el resultado anterior a su sitio;
/// quita las áreas temporales y conserva solo los puntos de control.
pub fn recover_work(data: &Path) {
    let Ok(entries) = fs::read_dir(data.join("work")) else {
        return;
    };
    for entry in entries.flatten() {
        let work = entry.path();
        let id = entry.file_name().to_string_lossy().into_owned();
        let previous = work.join("previous");
        let target = result_dir(data, &id);
        if previous.is_dir() && !target.exists() {
            let _ = fs::create_dir_all(data.join("results"));
            let _ = fs::rename(&previous, &target);
        }
        for temporary in ["staging", "result", "previous"] {
            let _ = fs::remove_dir_all(work.join(temporary));
        }
        if !work.join("checkpoint").exists() {
            let _ = fs::remove_dir_all(&work);
        }
    }
}

#[derive(Serialize, Clone)]
pub struct Summary {
    pub seconds: f64,
    pub removed_margins: usize,
    pub joined_words: usize,
    pub headings: usize,
    pub images: usize,
    /// Leído como escaneo (con OCR).
    pub scanned: bool,
    /// Páginas dudosas del escaneo.
    pub doubtful_pages: usize,
    /// Páginas dudosas releídas con el modelo visual.
    pub reread_pages: usize,
    /// El modelo visual estaba disponible.
    pub visual_available: bool,
}

/// Títulos del resultado (los del lector y los que añade el pulido): líneas que empiezan por uno a
/// seis `#` y un espacio.
fn heading_count(markdown: &str) -> usize {
    markdown
        .lines()
        .filter(|line| {
            let hashes = line.chars().take_while(|c| *c == '#').count();
            (1..=6).contains(&hashes) && line[hashes..].starts_with(' ')
        })
        .count()
}

/// Convierte y guarda el resultado de forma atómica.
pub fn run(
    data: &Path,
    id: &str,
    pdf: &Path,
    visual_full: bool,
    cancel: &AtomicBool,
) -> Result<Summary, String> {
    let pdfium = crate::pdf::pdfium().map_err(|e| e.message().to_string())?;
    let reader = reader(data)?;
    let work = work_dir(data, id);
    // Áreas temporales de un intento anterior fuera; los puntos de control se reutilizan.
    for temporary in ["staging", "result", "previous"] {
        let _ = fs::remove_dir_all(work.join(temporary));
    }
    fs::create_dir_all(&work).map_err(|_| "No se pudo preparar la carpeta de trabajo.")?;
    let outcome = (|| {
        let visual = visual_reader(data);
        let converted = if is_pdf(pdf) {
            convert_pdf(
                pdfium,
                &reader,
                pdf,
                &work.join("staging"),
                &work.join("checkpoint"),
                Visual {
                    reader: visual.as_ref(),
                    full: visual_full,
                },
                cancel,
            )
        } else {
            convert_other(&reader, pdf, &work.join("staging"), cancel)
        }
        .map_err(|e| e.message())?;
        let pending = work.join("result");
        fs::create_dir_all(pending.join("images"))
            .map_err(|_| "No se pudo guardar el resultado.")?;
        fs::write(pending.join("document.md"), &converted.markdown)
            .map_err(|_| "No se pudo guardar el resultado.")?;
        fs::write(
            pending.join("review.json"),
            serde_json::to_string(&converted.review).unwrap_or_default(),
        )
        .map_err(|_| "No se pudo guardar el resultado.")?;
        for image in &converted.images {
            if let Some(name) = image.file_name() {
                fs::copy(image, pending.join("images").join(name))
                    .map_err(|_| "No se pudo guardar una imagen.")?;
            }
        }
        let summary = Summary {
            seconds: (converted.seconds * 10.0).round() / 10.0,
            removed_margins: converted.report.removed_margins,
            joined_words: converted.report.joined_words,
            headings: heading_count(&converted.markdown),
            images: converted.images.len(),
            scanned: converted.scanned,
            doubtful_pages: converted.doubtful_pages.len(),
            reread_pages: converted.reread_pages.len(),
            visual_available: converted.visual_available,
        };
        fs::write(
            pending.join("summary.json"),
            serde_json::to_string(&summary).unwrap_or_default(),
        )
        .map_err(|_| "No se pudo guardar el resultado.")?;
        // Sustitución atómica del resultado anterior: se conserva hasta tener el nuevo.
        let target = result_dir(data, id);
        fs::create_dir_all(target.parent().unwrap_or(data))
            .map_err(|_| "No se pudo guardar el resultado.")?;
        let previous = work.join("previous");
        if target.exists() {
            fs::rename(&target, &previous).map_err(|_| "No se pudo sustituir el resultado.")?;
        }
        if fs::rename(&pending, &target).is_err() {
            if previous.exists() {
                let _ = fs::rename(&previous, &target);
            }
            return Err("No se pudo sustituir el resultado.".to_string());
        }
        Ok(summary)
    })();
    if outcome.is_ok() {
        // Publicado: ya no hace falta reanudar nada.
        let _ = fs::remove_dir_all(&work);
    } else {
        // Fallo o cancelación: se conservan los puntos de control para reanudar.
        for temporary in ["staging", "result"] {
            let _ = fs::remove_dir_all(work.join(temporary));
        }
    }
    outcome
}

/// Una página del resultado, ya convertida a HTML seguro para mostrarla.
#[derive(Serialize)]
pub struct PageView {
    pub page: u32,
    pub html: String,
}

/// HTML por página. El HTML incrustado se muestra como texto y las imágenes van embebidas.
pub fn pages(data: &Path, id: &str, translation: Option<&str>) -> Result<Vec<PageView>, String> {
    let folder = result_dir(data, id);
    let markdown = fs::read_to_string(document_path(data, id, translation))
        .map_err(|_| "Este documento todavía no tiene resultado.")?;
    let mut result = Vec::new();
    let mut page = 0;
    let mut buffer = String::new();
    let flush = |page: u32, buffer: &mut String, result: &mut Vec<PageView>| {
        if page > 0 || !buffer.trim().is_empty() {
            result.push(PageView {
                page: page.max(1),
                html: buffer.clone(),
            });
        }
        buffer.clear();
    };
    let images = folder.join("images");
    let mut index = 0;
    for block in markdown.split("\n\n") {
        if let Some(rest) = block.trim().strip_prefix("<!-- page:") {
            flush(page, &mut buffer, &mut result);
            page = rest
                .trim_end_matches("-->")
                .trim()
                .parse()
                .unwrap_or(page + 1);
            index = 0;
            continue;
        }
        // Cada fragmento de contenido lleva su número, el mismo que usa la lista de revisión.
        if is_content_block(block) {
            buffer.push_str(&format!(
                "<div class=\"block\" data-block=\"{index}\">{}</div>",
                to_html(block, &images)
            ));
            index += 1;
        } else {
            buffer.push_str(&to_html(block, &images));
        }
    }
    flush(page, &mut buffer, &mut result);
    Ok(result)
}

/// Vista previa de un Markdown escrito en el editor, con las imágenes del resultado.
pub fn preview(data: &Path, id: &str, markdown: &str) -> String {
    to_html(
        &markdown.replace("\r\n", "\n"),
        &result_dir(data, id).join("images"),
    )
}

/// Muestra un archivo seleccionado en el Explorador de Windows (o abre la carpeta).
pub fn reveal(path: &Path) -> Result<(), String> {
    let mut command = std::process::Command::new("explorer");
    if path.is_file() {
        command.arg(format!("/select,{}", path.display()));
    } else if path.is_dir() {
        command.arg(path);
    } else {
        return Err("Ese archivo ya no está en su sitio.".into());
    }
    command
        .spawn()
        .map(|_| ())
        .map_err(|_| "No se pudo abrir el Explorador.".into())
}

fn to_html(markdown: &str, images: &Path) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_MATH);
    let events = Parser::new_ext(markdown, options).map(|event| match event {
        Event::Html(raw) | Event::InlineHtml(raw) => Event::Text(raw),
        Event::InlineMath(_) | Event::DisplayMath(_) => math_event(event),
        Event::Start(Tag::Image {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            let name = dest_url.rsplit(['/', '\\']).next().unwrap_or("");
            let source = fs::read(images.join(name))
                .map(|bytes| {
                    let kind = if name.to_lowercase().ends_with(".png") {
                        "image/png"
                    } else {
                        "image/jpeg"
                    };
                    format!("data:{kind};base64,{}", base64(&bytes))
                })
                .unwrap_or_default();
            Event::Start(Tag::Image {
                link_type,
                dest_url: CowStr::from(source),
                title,
                id,
            })
        }
        other => other,
    });
    let mut out = String::new();
    html::push_html(&mut out, events);
    out
}

fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let n = (u32::from(chunk[0]) << 16)
            | (u32::from(*chunk.get(1).unwrap_or(&0)) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[((n >> (18 - 6 * i)) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Escribe en `path` pasando por un temporal: o queda el archivo completo o no queda nada.
/// Un aviso de la lista de revisión, con sus motivos en palabras y la decisión tomada.
#[derive(Serialize)]
pub struct ReviewEntry {
    pub page: u32,
    pub block: Option<usize>,
    pub key: String,
    pub excerpt: String,
    pub risk: f64,
    pub reasons: Vec<String>,
    pub decision: Option<String>,
}

#[derive(Serialize)]
pub struct ReviewView {
    pub items: Vec<ReviewEntry>,
    /// Riesgo de todos los avisos y de los que siguen sin decidir (o aplazados).
    pub total_risk: f64,
    pub pending_risk: f64,
}

fn reason_text(reason: &Reason) -> String {
    match reason {
        Reason::UnsupportedWords { examples, .. } => {
            format!("Palabras que el PDF no tiene: {}", examples.join(", "))
        }
        Reason::OddTokens { examples, .. } => {
            format!("Caracteres o palabras raras: {}", examples.join(", "))
        }
        Reason::SuspectWords { examples, .. } => {
            format!("Posibles errores de lectura: {}", examples.join(", "))
        }
        Reason::DoubtfulPage => {
            "Página dudosa sin releer: el OCR y la capa de texto del escaneo no coinciden".into()
        }
        Reason::VisualReread => {
            "Página releída por el modelo visual: compruébala junto al original".into()
        }
    }
}

/// Lista de revisión de un resultado con las decisiones tomadas. Si el resultado es anterior a la
/// lista (sin `review.json`), se calcula ahora con el texto del PDF.
pub fn review_view(
    data: &Path,
    id: &str,
    pdf: &Path,
    decisions: &HashMap<String, String>,
) -> Result<ReviewView, String> {
    let folder = result_dir(data, id);
    let items: Vec<ReviewItem> = match fs::read(folder.join("review.json")) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|_| "La lista de revisión está dañada.")?
        }
        Err(_) => {
            let markdown = fs::read_to_string(folder.join("document.md"))
                .map_err(|_| "Este documento todavía no tiene resultado.")?;
            let pdfium = crate::pdf::pdfium().map_err(|e| e.message().to_string())?;
            let texts = liblevo_worker::pdf_styles::page_texts(pdfium, pdf);
            let items = review::review(&markdown, &texts, &PageStates::default());
            // Se guarda: la lista queda fija aunque luego se corrijan fragmentos.
            let json = serde_json::to_vec(&items).map_err(|_| "No se pudo guardar la lista.")?;
            write_atomic(&folder.join("review.json"), &json)?;
            items
        }
    };
    let total_risk = items.iter().map(|i| i.risk).sum();
    let pending_risk = items
        .iter()
        .filter(|i| decisions.get(&i.key).is_none_or(|d| d == "despues"))
        .map(|i| i.risk)
        .sum();
    Ok(ReviewView {
        items: items
            .into_iter()
            .map(|i| ReviewEntry {
                decision: decisions.get(&i.key).cloned(),
                reasons: i.reasons.iter().map(reason_text).collect(),
                page: i.page,
                block: i.block,
                key: i.key,
                excerpt: i.excerpt,
                risk: i.risk,
            })
            .collect(),
        total_risk,
        pending_risk,
    })
}

/// Avisos de revisión (total y sin decidir) de un resultado ya guardado, sin calcular la lista si
/// todavía no existe.
pub fn review_counts(
    data: &Path,
    id: &str,
    decisions: &HashMap<String, String>,
) -> (Option<usize>, usize) {
    let Ok(bytes) = fs::read(result_dir(data, id).join("review.json")) else {
        return (None, 0);
    };
    let items: Vec<ReviewItem> = serde_json::from_slice(&bytes).unwrap_or_default();
    let pending = items
        .iter()
        .filter(|i| decisions.get(&i.key).is_none_or(|d| d == "despues"))
        .count();
    (Some(items.len()), pending)
}

/// Posición (inicio, fin) del fragmento `block` de la página `page` en el Markdown.
fn locate_block(markdown: &str, page: u32, block: usize) -> Option<(usize, usize)> {
    let mut current = 0;
    let mut index = 0;
    let mut offset = 0;
    for raw in markdown.split("\n\n") {
        let start = offset;
        offset += raw.len() + 2;
        if let Some(rest) = raw.trim().strip_prefix("<!-- page:") {
            current = rest
                .trim_end_matches("-->")
                .trim()
                .parse()
                .unwrap_or(current + 1);
            index = 0;
            continue;
        }
        if current == page && is_content_block(raw) {
            if index == block {
                let trimmed = raw.trim();
                let inner = start + raw.find(trimmed).unwrap_or(0);
                return Some((inner, inner + trimmed.len()));
            }
            index += 1;
        }
    }
    None
}

/// Texto original (Markdown) de un fragmento, para corregirlo.
pub fn block_text(data: &Path, id: &str, page: u32, block: usize) -> Result<String, String> {
    let markdown = fs::read_to_string(result_dir(data, id).join("document.md"))
        .map_err(|_| "Este documento todavía no tiene resultado.")?;
    locate_block(&markdown, page, block)
        .map(|(start, end)| markdown[start..end].to_string())
        .ok_or_else(|| "No se encuentra ese fragmento en el resultado.".to_string())
}

/// Sustituye el texto de un fragmento del resultado, solo si sigue siendo el revisado (misma
/// huella). El texto nuevo no puede partir el fragmento ni crear páginas.
pub fn correct_block(
    data: &Path,
    id: &str,
    page: u32,
    block: usize,
    key: &str,
    text: &str,
) -> Result<(), String> {
    let path = result_dir(data, id).join("document.md");
    let markdown =
        fs::read_to_string(&path).map_err(|_| "Este documento todavía no tiene resultado.")?;
    let (start, end) = locate_block(&markdown, page, block)
        .ok_or("No se encuentra ese fragmento en el resultado.")?;
    if block_key(&markdown[start..end]) != key {
        return Err("Ese fragmento ha cambiado desde que se hizo la lista.".into());
    }
    let replacement: String = text
        .replace("\r\n", "\n")
        .lines()
        .map(str::trim_end)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    if replacement.trim().is_empty() || replacement.contains("<!-- page:") {
        return Err(
            "El texto corregido no puede quedar vacío ni contener marcas de página.".into(),
        );
    }
    let updated = format!(
        "{}{}{}",
        &markdown[..start],
        replacement.trim(),
        &markdown[end..]
    );
    write_atomic(&path, updated.as_bytes())
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temporary = path.with_extension("liblevo-tmp");
    fs::write(&temporary, bytes).map_err(|_| "No se pudo escribir el archivo.")?;
    fs::rename(&temporary, path).map_err(|_| {
        let _ = fs::remove_file(&temporary);
        "No se pudo escribir el archivo.".to_string()
    })
}

/// Páginas pedidas al exportar (primera y última, incluidas); `None` exporta todo.
pub type PageRange = Option<(u32, u32)>;

/// Deja solo las páginas del intervalo (con sus marcas). Un resultado sin marcas (otros formatos)
/// se deja entero.
fn only_pages(markdown: &str, range: PageRange) -> Result<String, String> {
    let Some((first, last)) = range else {
        return Ok(markdown.to_string());
    };
    if first == 0 || last < first {
        return Err("El intervalo de páginas no es válido.".into());
    }
    if !markdown.contains("<!-- page:") {
        return Ok(markdown.to_string());
    }
    let mut page = 0u32;
    let kept: Vec<&str> = markdown
        .split("\n\n")
        .filter(|block| {
            if let Some(rest) = block.trim().strip_prefix("<!-- page:") {
                page = rest
                    .trim_end_matches("-->")
                    .trim()
                    .parse()
                    .unwrap_or(page + 1);
            }
            (first..=last).contains(&page)
        })
        .collect();
    if kept.iter().all(|block| block.trim().starts_with("<!--")) {
        return Err("Esas páginas no tienen texto en el resultado.".into());
    }
    Ok(kept.join("\n\n"))
}

/// Exporta el Markdown y copia sus imágenes a una carpeta `<nombre>-imagenes` al lado.
pub fn export_markdown(
    data: &Path,
    id: &str,
    translation: Option<&str>,
    range: PageRange,
    path: &Path,
) -> Result<(), String> {
    let folder = result_dir(data, id);
    let markdown = fs::read_to_string(document_path(data, id, translation))
        .map_err(|_| "Este documento todavía no tiene resultado.")?;
    let markdown = only_pages(&markdown, range)?;
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "documento".into());
    let images_name = format!("{stem}-imagenes");
    let mut text = without_page_markers(&markdown);
    let source_images = folder.join("images");
    let names: Vec<String> = fs::read_dir(&source_images)
        .map(|entries| {
            entries
                .filter_map(|e| e.ok()?.file_name().into_string().ok())
                .filter(|name| text.contains(&format!("]({name})")))
                .collect()
        })
        .unwrap_or_default();
    if !names.is_empty() {
        let target = path.with_file_name(&images_name);
        fs::create_dir_all(&target).map_err(|_| "No se pudo crear la carpeta de imágenes.")?;
        for name in &names {
            fs::copy(source_images.join(name), target.join(name))
                .map_err(|_| "No se pudo copiar una imagen.")?;
            text = text.replace(&format!("]({name})"), &format!("]({images_name}/{name})"));
        }
    }
    write_atomic(path, text.as_bytes())
}

pub fn export_epub(
    data: &Path,
    id: &str,
    translation: Option<&str>,
    range: PageRange,
    path: &Path,
    metadata: &BookMetadata,
) -> Result<(), String> {
    let folder = result_dir(data, id);
    let markdown = fs::read_to_string(document_path(data, id, translation))
        .map_err(|_| "Este documento todavía no tiene resultado.")?;
    let markdown = only_pages(&markdown, range)?;
    let images: Vec<ImageFile> = fs::read_dir(folder.join("images"))
        .map(|entries| {
            entries
                .filter_map(|e| {
                    let entry = e.ok()?;
                    Some(ImageFile {
                        name: entry.file_name().into_string().ok()?,
                        bytes: fs::read(entry.path()).ok()?,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    let bytes = write_markdown_epub(
        &without_page_markers(&markdown),
        metadata,
        &images,
        std::io::Cursor::new(Vec::new()),
    )
    .map_err(|_| "No se pudo crear el EPUB.")?
    .into_inner();
    write_atomic(path, &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translations_are_listed_shown_and_exported_beside_the_result() {
        let data = std::env::temp_dir().join(format!("liblevo-tradapp-{}", std::process::id()));
        let _ = fs::remove_dir_all(&data);
        let folder = result_dir(&data, "d");
        fs::create_dir_all(folder.join("images")).unwrap();
        fs::write(folder.join("images").join("a.png"), b"png").unwrap();
        fs::write(
            folder.join("document.md"),
            "<!-- page: 1 -->

Hello.

![](a.png)",
        )
        .unwrap();
        let es = translation_dir(&data, "d", "es");
        fs::create_dir_all(&es).unwrap();
        fs::write(
            es.join("document.md"),
            "<!-- page: 1 -->

Hola.

![](a.png)",
        )
        .unwrap();
        let summary = TranslationSummary {
            language: "es".into(),
            label: "Español".into(),
            quality: false,
            blocks: 1,
            doubts: 0,
            seconds: 1.0,
            untouched: 0,
        };
        fs::write(
            es.join("summary.json"),
            serde_json::to_string(&summary).unwrap(),
        )
        .unwrap();
        let listed = translations(&data, "d");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].label, "Español");
        let views = pages(&data, "d", Some("es")).unwrap();
        assert!(views[0].html.contains("Hola") && views[0].html.contains("data:image"));
        let md = data.join("salida.md");
        export_markdown(&data, "d", Some("es"), None, &md).unwrap();
        assert!(fs::read_to_string(&md).unwrap().starts_with("Hola."));
        let pages = "<!-- page: 1 -->\n\nUno.\n\n<!-- page: 2 -->\n\n![](a.png)\n\nDos.\n\n<!-- page: 3 -->\n\nTres.";
        assert_eq!(
            only_pages(pages, Some((2, 2))).unwrap(),
            "<!-- page: 2 -->\n\n![](a.png)\n\nDos."
        );
        assert_eq!(only_pages(pages, None).unwrap(), pages);
        assert!(only_pages(pages, Some((3, 2))).is_err());
        assert!(only_pages(pages, Some((7, 9))).is_err());
        assert_eq!(
            only_pages("Sin páginas.", Some((2, 3))).unwrap(),
            "Sin páginas."
        );
        assert!(data.join("salida-imagenes").join("a.png").is_file());
        assert_eq!(
            parse_glossary(
                "press = imprenta

 ink=tinta "
            )
            .unwrap(),
            vec![
                ("press".to_string(), "imprenta".to_string()),
                ("ink".to_string(), "tinta".to_string())
            ]
        );
        assert!(parse_glossary("sin signo").is_err());
        assert!(parse_glossary("= nada").is_err());
        fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn the_page_editor_replaces_only_its_page() {
        let data = std::env::temp_dir().join(format!("liblevo-editor-{}", std::process::id()));
        let _ = fs::remove_dir_all(&data);
        let folder = result_dir(&data, "d");
        fs::create_dir_all(&folder).unwrap();
        fs::write(
            folder.join("document.md"),
            "<!-- page: 1 -->\n\nUno.\n\n<!-- page: 2 -->\n\nDos mal.\n\n<!-- page: 3 -->\n\nTres.",
        )
        .unwrap();
        assert_eq!(page_markdown(&data, "d", 2).unwrap(), "Dos mal.");
        assert!(save_page(&data, "d", 2, "x <!-- page: 9 -->").is_err());
        assert!(save_page(&data, "d", 7, "x").is_err());
        save_page(&data, "d", 2, "# Dos\r\n\r\nBien.").unwrap();
        let saved = fs::read_to_string(folder.join("document.md")).unwrap();
        assert_eq!(
            saved,
            "<!-- page: 1 -->\n\nUno.\n\n<!-- page: 2 -->\n\n# Dos\n\nBien.\n\n<!-- page: 3 -->\n\nTres."
        );
        assert!(is_pdf(Path::new("a.PDF")) && !is_pdf(Path::new("a.docx")));
        fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn a_reviewed_block_is_corrected_only_if_it_did_not_change() {
        let data = std::env::temp_dir().join(format!("liblevo-correct-{}", std::process::id()));
        let _ = fs::remove_dir_all(&data);
        let folder = result_dir(&data, "d");
        fs::create_dir_all(&folder).unwrap();
        let markdown = "<!-- page: 1 -->\n\n# Título\n\n![](a.png)\n\nUno mal leido.\n\n\
            <!-- page: 2 -->\n\nDos.";
        fs::write(folder.join("document.md"), markdown).unwrap();
        // Fragmento 1 de la página 1 (la imagen no cuenta).
        let original = block_text(&data, "d", 1, 1).unwrap();
        assert_eq!(original, "Uno mal leido.");
        let key = block_key(&original);
        assert!(correct_block(&data, "d", 1, 1, "otra-huella", "x").is_err());
        assert!(correct_block(&data, "d", 1, 1, &key, "\n\n").is_err());
        correct_block(&data, "d", 1, 1, &key, "Uno bien leído.\n\n").unwrap();
        let fixed = fs::read_to_string(folder.join("document.md")).unwrap();
        assert_eq!(fixed, markdown.replace("Uno mal leido.", "Uno bien leído."));
        // La vista numera los mismos fragmentos.
        let views = pages(&data, "d", None).unwrap();
        assert!(
            views[0]
                .html
                .contains("data-block=\"1\"><p>Uno bien leído.</p>")
        );
        assert!(views[1].html.contains("data-block=\"0\"><p>Dos.</p>"));
        fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn startup_recovery_restores_results_and_keeps_only_checkpoints() {
        let data = std::env::temp_dir().join(format!("liblevo-recover-{}", std::process::id()));
        let _ = fs::remove_dir_all(&data);
        // Documento «a»: cierre entre los dos cambios de nombre; su resultado está en `previous`.
        fs::create_dir_all(data.join("work/a/previous")).unwrap();
        fs::write(data.join("work/a/previous/document.md"), "anterior").unwrap();
        fs::create_dir_all(data.join("work/a/staging")).unwrap();
        // Documento «b»: interrumpido con puntos de control.
        liblevo_worker::checkpoint::Checkpoint::open(&data.join("work/b/checkpoint"), "id")
            .unwrap();
        fs::create_dir_all(data.join("work/b/staging")).unwrap();
        recover_work(&data);
        assert_eq!(
            fs::read_to_string(result_dir(&data, "a").join("document.md")).unwrap(),
            "anterior"
        );
        assert!(!work_dir(&data, "a").exists());
        assert!(resumable(&data, "b"));
        assert!(!work_dir(&data, "b").join("staging").exists());
        fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn headings_are_counted_in_the_result() {
        let markdown = "# Uno

Texto #no

## Dos

####### siete

#sin espacio";
        assert_eq!(heading_count(markdown), 2);
    }

    /// Recorrido real: convierte, guarda, muestra páginas y exporta. Solo se ejecuta si están
    /// definidos los componentes (`LIBLEVO_PDFIUM`, `LIBLEVO_XBERG`, `LIBLEVO_XBERG_MODELS`) y
    /// un PDF de prueba (`LIBLEVO_TEST_PDF`).
    #[test]
    fn real_conversion_is_saved_shown_and_exported_when_components_exist() {
        let Some(pdf) = std::env::var_os("LIBLEVO_TEST_PDF").map(PathBuf::from) else {
            eprintln!("LIBLEVO_TEST_PDF no está definido: se omite el recorrido real");
            return;
        };
        let data = std::env::temp_dir().join(format!("liblevo-e2e-{}", std::process::id()));
        let _ = fs::remove_dir_all(&data);
        let summary = run(&data, "doc", &pdf, false, &AtomicBool::new(false)).unwrap();
        assert!(summary.seconds > 0.0);
        let views = pages(&data, "doc", None).unwrap();
        assert!(!views.is_empty());
        assert!(views.iter().any(|v| v.html.contains("<p>")));
        let md = data.join("salida.md");
        export_markdown(&data, "doc", None, None, &md).unwrap();
        assert!(!fs::read_to_string(&md).unwrap().contains("<!-- page:"));
        let epub = data.join("salida.epub");
        let metadata = BookMetadata {
            title: "Prueba".into(),
            author: None,
            language: "en".into(),
            identifier: "urn:liblevo:prueba".into(),
            modified: "2026-10-19T00:00:00Z".into(),
            cover: None,
        };
        export_epub(&data, "doc", None, None, &epub, &metadata).unwrap();
        assert!(fs::read(&epub).unwrap().starts_with(b"PK"));
        assert!(!data.join("work").join("doc").exists());
        fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn base64_matches_known_values() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn pages_split_by_markers_and_escape_html() {
        let data = std::env::temp_dir().join(format!("liblevo-conv-{}", std::process::id()));
        let folder = result_dir(&data, "doc");
        fs::create_dir_all(folder.join("images")).unwrap();
        fs::write(
            folder.join("document.md"),
            "<!-- page: 1 -->\n\n# Título\n\n<script>x</script>\n\n<!-- page: 2 -->\n\nTexto *dos*.",
        )
        .unwrap();
        let views = pages(&data, "doc", None).unwrap();
        assert_eq!(views.len(), 2);
        assert_eq!(views[0].page, 1);
        assert!(views[0].html.contains("<h1>Título</h1>"));
        assert!(!views[0].html.contains("<script"));
        assert!(views[1].html.contains("<em>dos</em>"));
        fs::remove_dir_all(&data).unwrap();
    }

    #[test]
    fn markdown_export_copies_only_referenced_images_and_rewrites_links() {
        let data = std::env::temp_dir().join(format!("liblevo-exp-{}", std::process::id()));
        let folder = result_dir(&data, "doc");
        fs::create_dir_all(folder.join("images")).unwrap();
        fs::write(folder.join("images/fig.png"), b"png").unwrap();
        fs::write(folder.join("images/otra.png"), b"png").unwrap();
        fs::write(
            folder.join("document.md"),
            "<!-- page: 1 -->\n\n# Libro\n\n![Figura](fig.png)\n",
        )
        .unwrap();
        let out = data.join("salida").join("libro.md");
        fs::create_dir_all(out.parent().unwrap()).unwrap();
        export_markdown(&data, "doc", None, None, &out).unwrap();
        let text = fs::read_to_string(&out).unwrap();
        assert_eq!(text, "# Libro\n\n![Figura](libro-imagenes/fig.png)\n");
        assert!(data.join("salida/libro-imagenes/fig.png").exists());
        assert!(!data.join("salida/libro-imagenes/otra.png").exists());
        fs::remove_dir_all(&data).unwrap();
    }
}
