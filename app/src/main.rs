//! Aplicación de escritorio Liblevo.
//!
//! Recuerda la lista de documentos (SQLite), convierte, revisa y traduce en segundo plano con el
//! motor, muestra cada página junto a su texto, descarga componentes verificados y exporta a
//! Markdown y EPUB.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod components;
mod conversion;
mod pdf;
mod store;

use conversion::{Conversions, PageView, Summary, TranslationSummary};
use liblevo_core::markdown_epub::BookMetadata;
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use store::{Store, StoredDocument};
use tauri::{AppHandle, Emitter, Manager, State, ipc::Response};

struct AppState {
    store: Mutex<Store>,
    data: PathBuf,
    conversions: Conversions,
    /// Descargas de componentes en curso, con su señal de cancelación.
    installs: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

#[derive(Serialize)]
struct ComponentView {
    id: String,
    name: String,
    description: String,
    license: String,
    required: bool,
    megabytes: u64,
    installed: bool,
    installing: bool,
}

/// Aviso a la interfaz sobre una descarga de componente.
#[derive(Serialize, Clone)]
struct ComponentEvent {
    id: String,
    state: &'static str,
    done: u64,
    total: u64,
    message: Option<String>,
}

#[derive(Serialize)]
struct DocumentView {
    id: String,
    name: String,
    pages: u32,
    /// El original sigue en su sitio. Si no, la app lo indica en vez de fallar al abrirlo.
    available: bool,
    /// Hay un resultado guardado de una conversión anterior.
    converted: bool,
    converting: bool,
    /// Hay una conversión interrumpida que se puede reanudar.
    resumable: bool,
    /// Releer todas las páginas del escaneo con el modelo visual.
    visual_full: bool,
    /// Es un PDF (tiene páginas que se pueden dibujar junto al resultado).
    paged: bool,
    translating: bool,
    /// Traducciones guardadas del resultado.
    translations: Vec<TranslationSummary>,
    /// Ruta del original, para mostrarlo en su carpeta.
    path: String,
    /// Avisos de revisión del resultado y cuántos siguen sin decidir.
    review_total: Option<usize>,
    review_pending: usize,
}

/// Aviso a la interfaz sobre una traducción (sin contenido documental).
#[derive(Serialize, Clone)]
struct TranslationEvent {
    id: String,
    state: &'static str,
    done: usize,
    total: usize,
    message: Option<String>,
    summary: Option<TranslationSummary>,
    /// Se tradujo el EPUB original (su copia de trabajo), no el resultado.
    epub: bool,
}

#[derive(Serialize)]
struct TranslationOptions {
    /// (código, nombre en la interfaz).
    languages: Vec<(String, String)>,
    fast: bool,
    quality: bool,
}

/// Clave de la traducción en curso de un documento (comparte el registro de tareas).
fn translation_key(id: &str) -> String {
    format!("{id}#traduccion")
}

/// Aviso a la interfaz sobre una conversión (sin contenido documental).
#[derive(Serialize, Clone)]
struct ConversionEvent {
    id: String,
    state: &'static str,
    message: Option<String>,
    summary: Option<Summary>,
}

const STATE_ERROR: &str = "No se pudo acceder a la lista de documentos.";

fn view(state: &AppState, document: StoredDocument) -> DocumentView {
    let decisions = state
        .store
        .lock()
        .ok()
        .and_then(|store| store.decisions(&document.id).ok())
        .unwrap_or_default();
    let (review_total, review_pending) =
        conversion::review_counts(&state.data, &document.id, &decisions);
    DocumentView {
        path: document.path.display().to_string(),
        review_total,
        review_pending,
        available: document.path.is_file(),
        converted: conversion::result_dir(&state.data, &document.id)
            .join("document.md")
            .is_file(),
        converting: state.conversions.is_running(&document.id),
        resumable: !state.conversions.is_running(&document.id)
            && conversion::resumable(&state.data, &document.id),
        visual_full: document.visual_full,
        paged: conversion::is_pdf(&document.path),
        translating: state.conversions.is_running(&translation_key(&document.id)),
        translations: conversion::translations(&state.data, &document.id),
        id: document.id,
        name: document.name,
        pages: document.pages,
    }
}

fn stored(state: &AppState, id: &str) -> Result<StoredDocument, String> {
    let store = state.store.lock().map_err(|_| STATE_ERROR.to_string())?;
    store
        .get(id)
        .map_err(|_| STATE_ERROR.to_string())?
        .ok_or_else(|| "El documento ya no está en la lista.".to_string())
}

#[tauri::command]
fn list_documents(state: State<'_, AppState>) -> Result<Vec<DocumentView>, String> {
    let documents = {
        let store = state.store.lock().map_err(|_| STATE_ERROR.to_string())?;
        store.list().map_err(|_| STATE_ERROR.to_string())?
    };
    Ok(documents.into_iter().map(|d| view(&state, d)).collect())
}

#[tauri::command]
fn open_document(path: String, state: State<'_, AppState>) -> Result<DocumentView, String> {
    let path = PathBuf::from(path);
    if !liblevo_worker::convert::supported(&path) {
        return Err("Formato no admitido: PDF, DOCX, EPUB, HTML, TXT o Markdown.".into());
    }
    // Un PDF se inspecciona (páginas, huella); los demás formatos son una sola «página».
    let (id, pages) = if conversion::is_pdf(&path) {
        let inspected = pdf::inspect(&path).map_err(|e| e.message().to_string())?;
        (inspected.sha256, inspected.pages)
    } else {
        let metadata = std::fs::metadata(&path).map_err(|_| "No se encuentra el archivo.")?;
        if metadata.len() > 512 * 1024 * 1024 {
            return Err("El archivo es demasiado grande.".into());
        }
        let sha = liblevo_worker::xberg_reader::sha256_file(&path)
            .map_err(|_| "No se pudo leer el archivo.")?;
        (sha, 1)
    };
    let document = StoredDocument {
        id,
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        path,
        pages,
        visual_full: false,
    };
    {
        let store = state.store.lock().map_err(|_| STATE_ERROR.to_string())?;
        store.add(&document).map_err(|_| STATE_ERROR.to_string())?;
    }
    Ok(view(&state, document))
}

#[tauri::command]
fn remove_document(id: String, state: State<'_, AppState>) -> Result<(), String> {
    state.conversions.cancel(&id);
    state.conversions.cancel(&translation_key(&id));
    let store = state.store.lock().map_err(|_| STATE_ERROR.to_string())?;
    store.remove(&id).map_err(|_| STATE_ERROR.to_string())?;
    // El resultado guardado y lo pendiente se borran con el documento; el original nunca se toca.
    let _ = std::fs::remove_dir_all(conversion::result_dir(&state.data, &id));
    let _ = std::fs::remove_dir_all(conversion::work_dir(&state.data, &id));
    conversion::epub_discard(&state.data, &id);
    let _ = std::fs::remove_dir_all(conversion::epub_translation_cache(&state.data, &id));
    Ok(())
}

#[tauri::command]
fn render_page(
    id: String,
    page: u32,
    width: u32,
    state: State<'_, AppState>,
) -> Result<Response, String> {
    let document = stored(&state, &id)?;
    let png = pdf::render_png(&document.path, page, width.clamp(200, 2400))
        .map_err(|e| e.message().to_string())?;
    Ok(Response::new(png))
}

#[tauri::command]
fn convert_document(id: String, app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let document = stored(&state, &id)?;
    if !document.path.is_file() {
        return Err("El original ya no está en su carpeta.".into());
    }
    if state.conversions.is_running(&translation_key(&id)) {
        return Err("Espera a que termine la traducción o cancélala antes de convertir.".into());
    }
    let cancel = state
        .conversions
        .start(&id)
        .ok_or("Este documento ya se está convirtiendo.")?;
    let data = state.data.clone();
    std::thread::spawn(move || {
        let _ = app.emit(
            "conversion",
            ConversionEvent {
                id: id.clone(),
                state: "running",
                message: None,
                summary: None,
            },
        );
        let outcome = conversion::run(&data, &id, &document.path, document.visual_full, &cancel);
        let cancelled = cancel.load(std::sync::atomic::Ordering::Relaxed);
        if let Some(state) = app.try_state::<AppState>() {
            state.conversions.finish(&id);
        }
        if outcome.is_ok()
            && let Some(state) = app.try_state::<AppState>()
            && let Ok(store) = state.store.lock()
        {
            // Resultado nuevo: las decisiones de revisión del anterior ya no aplican.
            let _ = store.clear_decisions(&id);
        }
        let event = match outcome {
            Ok(summary) => ConversionEvent {
                id,
                state: "done",
                message: None,
                summary: Some(summary),
            },
            Err(_) if cancelled => ConversionEvent {
                id,
                state: "cancelled",
                message: None,
                summary: None,
            },
            Err(message) => ConversionEvent {
                id,
                state: "failed",
                message: Some(message),
                summary: None,
            },
        };
        let _ = app.emit("conversion", event);
    });
    Ok(())
}

#[tauri::command]
fn review_list(id: String, state: State<'_, AppState>) -> Result<conversion::ReviewView, String> {
    let document = stored(&state, &id)?;
    let decisions = {
        let store = state.store.lock().map_err(|_| STATE_ERROR.to_string())?;
        store.decisions(&id).map_err(|_| STATE_ERROR.to_string())?
    };
    conversion::review_view(&state.data, &id, &document.path, &decisions)
}

#[tauri::command]
fn review_block_text(
    id: String,
    page: u32,
    block: usize,
    state: State<'_, AppState>,
) -> Result<String, String> {
    conversion::block_text(&state.data, &id, page, block)
}

/// Guarda la decisión sobre un aviso: «correcto», «despues» o «corregido» (con el texto nuevo, que
/// se aplica al resultado).
#[tauri::command]
fn review_decide(
    id: String,
    key: String,
    decision: String,
    page: u32,
    block: Option<usize>,
    text: Option<String>,
    state: State<'_, AppState>,
) -> Result<conversion::ReviewView, String> {
    if !["correcto", "corregido", "despues"].contains(&decision.as_str()) {
        return Err("Decisión desconocida.".into());
    }
    if state.conversions.is_running(&id) {
        return Err("Espera a que termine la conversión.".into());
    }
    if decision == "corregido" {
        let block = block.ok_or("Solo se puede corregir un fragmento concreto.")?;
        let text = text.ok_or("Falta el texto corregido.")?;
        conversion::correct_block(&state.data, &id, page, block, &key, &text)?;
    }
    {
        let store = state.store.lock().map_err(|_| STATE_ERROR.to_string())?;
        store
            .decide(&id, &key, &decision)
            .map_err(|_| STATE_ERROR.to_string())?;
    }
    review_list(id, state)
}

#[tauri::command]
fn page_markdown(id: String, page: u32, state: State<'_, AppState>) -> Result<String, String> {
    conversion::page_markdown(&state.data, &id, page)
}

/// Editor mínimo: sustituye el texto (Markdown) de una página del resultado.
#[tauri::command]
fn save_page(
    id: String,
    page: u32,
    text: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if state.conversions.is_running(&id) {
        return Err("Espera a que termine la conversión.".into());
    }
    conversion::save_page(&state.data, &id, page, &text)
}

/// Vista previa del editor de página.
#[tauri::command]
fn preview_markdown(id: String, text: String, state: State<'_, AppState>) -> String {
    conversion::preview(&state.data, &id, &text)
}

/// Muestra en el Explorador un archivo exportado.
#[tauri::command]
fn reveal_path(path: String) -> Result<(), String> {
    conversion::reveal(&PathBuf::from(path))
}

/// Abre la carpeta de datos de la app.
#[tauri::command]
fn reveal_data(state: State<'_, AppState>) -> Result<(), String> {
    conversion::reveal(&state.data)
}

/// Abre un EPUB guardado en Sigil o, si no está, en el editor de Calibre.
#[tauri::command]
fn open_in_epub_editor(path: String) -> Result<String, String> {
    conversion::open_in_epub_editor(&PathBuf::from(path))
}

#[tauri::command]
fn epub_chapter(
    id: String,
    chapter: usize,
    state: State<'_, AppState>,
) -> Result<conversion::EpubChapterView, String> {
    let document = stored(&state, &id)?;
    conversion::epub_chapter(&state.data, &id, &document.path, chapter)
}

#[tauri::command]
fn epub_set_text(
    id: String,
    chapter: usize,
    index: usize,
    text: String,
    state: State<'_, AppState>,
) -> Result<conversion::EpubChapterView, String> {
    let document = stored(&state, &id)?;
    conversion::epub_set_text(&state.data, &id, &document.path, chapter, index, &text)?;
    conversion::epub_chapter(&state.data, &id, &document.path, chapter)
}

#[tauri::command]
fn epub_save_as(id: String, path: String, state: State<'_, AppState>) -> Result<(), String> {
    let document = stored(&state, &id)?;
    conversion::epub_save_as(&state.data, &id, &document.path, &PathBuf::from(path))
}

#[tauri::command]
fn epub_discard(id: String, state: State<'_, AppState>) {
    conversion::epub_discard(&state.data, &id);
}

#[tauri::command]
fn set_visual_full(id: String, on: bool, state: State<'_, AppState>) -> Result<(), String> {
    let store = state.store.lock().map_err(|_| STATE_ERROR.to_string())?;
    store
        .set_visual_full(&id, on)
        .map_err(|_| STATE_ERROR.to_string())?;
    Ok(())
}

#[tauri::command]
fn cancel_conversion(id: String, state: State<'_, AppState>) -> bool {
    state.conversions.cancel(&id)
}

#[tauri::command]
fn translation_options(state: State<'_, AppState>) -> TranslationOptions {
    TranslationOptions {
        languages: liblevo_worker::translate::LANGUAGES
            .iter()
            .map(|(code, _, label)| ((*code).to_string(), (*label).to_string()))
            .collect(),
        fast: conversion::translator(&state.data, false).is_some(),
        quality: conversion::translator(&state.data, true).is_some(),
    }
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn translate_document(
    id: String,
    language: String,
    quality: bool,
    glossary: String,
    epub: bool,
    app: AppHandle,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let document = stored(&state, &id)?;
    if epub && !document.path.is_file() {
        return Err("El original ya no está en su carpeta.".into());
    }
    let glossary = conversion::parse_glossary(&glossary)?;
    if state.conversions.is_running(&id) {
        return Err("Espera a que termine la conversión antes de traducir.".into());
    }
    let key = translation_key(&id);
    let cancel = state
        .conversions
        .start(&key)
        .ok_or("Este documento ya se está traduciendo.")?;
    let data = state.data.clone();
    std::thread::spawn(move || {
        let event = |state: &'static str, done: usize, total: usize| TranslationEvent {
            id: id.clone(),
            state,
            done,
            total,
            message: None,
            summary: None,
            epub,
        };
        let _ = app.emit("translation", event("running", 0, 0));
        let mut progress = |done, total| {
            let _ = app.emit("translation", event("running", done, total));
        };
        let outcome = if epub {
            conversion::translate_epub_original(
                &data,
                &id,
                &document.path,
                &language,
                quality,
                &glossary,
                &cancel,
                &mut progress,
            )
        } else {
            conversion::translate(
                &data,
                &id,
                &language,
                quality,
                &glossary,
                &cancel,
                &mut progress,
            )
        };
        let cancelled = cancel.load(Ordering::Relaxed);
        if let Some(state) = app.try_state::<AppState>() {
            state.conversions.finish(&key);
        }
        let finished = match outcome {
            Ok(summary) => TranslationEvent {
                summary: Some(summary),
                ..event("done", 0, 0)
            },
            Err(_) if cancelled => event("cancelled", 0, 0),
            Err(message) => TranslationEvent {
                message: Some(message),
                ..event("failed", 0, 0)
            },
        };
        let _ = app.emit("translation", finished);
    });
    Ok(())
}

#[tauri::command]
fn cancel_translation(id: String, state: State<'_, AppState>) -> bool {
    state.conversions.cancel(&translation_key(&id))
}

#[tauri::command]
fn translation_doubts(
    id: String,
    language: String,
    state: State<'_, AppState>,
) -> Result<Vec<conversion::TranslationDoubtView>, String> {
    conversion::translation_doubts(&state.data, &id, &language)
}

#[tauri::command]
fn result_pages(
    id: String,
    translation: Option<String>,
    state: State<'_, AppState>,
) -> Result<Vec<PageView>, String> {
    conversion::pages(&state.data, &id, translation.as_deref())
}

#[tauri::command]
fn export_markdown(
    id: String,
    translation: Option<String>,
    pages: Option<(u32, u32)>,
    path: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    conversion::export_markdown(
        &state.data,
        &id,
        translation.as_deref(),
        pages,
        &PathBuf::from(path),
    )
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn export_epub(
    id: String,
    translation: Option<String>,
    pages: Option<(u32, u32)>,
    cover: bool,
    path: String,
    title: String,
    author: Option<String>,
    language: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("El EPUB necesita un título.".into());
    }
    let metadata = BookMetadata {
        title: title.to_string(),
        author: author
            .map(|a| a.trim().to_string())
            .filter(|a| !a.is_empty()),
        language: if language.trim().is_empty() {
            "und".into()
        } else {
            language.trim().to_string()
        },
        identifier: format!("urn:sha256:{id}"),
        modified: utc_now(),
        // Portada: la primera página del PDF, dibujada como en la vista del original.
        cover: if cover {
            let document = stored(&state, &id)?;
            conversion::is_pdf(&document.path)
                .then(|| pdf::render_png(&document.path, 1, 1200).ok())
                .flatten()
                .map(|bytes| liblevo_core::markdown_epub::ImageFile {
                    name: "portada.png".into(),
                    bytes,
                })
        } else {
            None
        },
    };
    conversion::export_epub(
        &state.data,
        &id,
        translation.as_deref(),
        pages,
        &PathBuf::from(path),
        &metadata,
    )
}

#[tauri::command]
fn list_components(state: State<'_, AppState>) -> Vec<ComponentView> {
    let root = conversion::components_dir(&state.data);
    let installing = state.installs.lock().map(|i| i.clone()).unwrap_or_default();
    components::catalog()
        .into_iter()
        .map(|c| ComponentView {
            installed: c.installed(&root),
            installing: installing.contains_key(&c.id),
            megabytes: c.size().div_ceil(1_000_000),
            id: c.id,
            name: c.name,
            description: c.description,
            license: c.license,
            required: c.required,
        })
        .collect()
}

#[tauri::command]
fn install_component(id: String, app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let component = components::catalog()
        .into_iter()
        .find(|c| c.id == id)
        .ok_or("Ese componente no está en el catálogo.")?;
    let cancel = Arc::new(AtomicBool::new(false));
    {
        let mut installs = state.installs.lock().map_err(|_| STATE_ERROR.to_string())?;
        if installs.contains_key(&id) {
            return Err("Ese componente ya se está descargando.".into());
        }
        installs.insert(id.clone(), cancel.clone());
    }
    let root = conversion::components_dir(&state.data);
    std::thread::spawn(move || {
        let event =
            |state: &'static str, done: u64, total: u64, message: Option<String>| ComponentEvent {
                id: id.clone(),
                state,
                done,
                total,
                message,
            };
        let mut last_sent = 0u64;
        let outcome = components::install(&root, &component, &cancel, &mut |done, total| {
            // Un aviso cada 5 MB como mucho.
            if done == total || done >= last_sent + 5_000_000 {
                last_sent = done;
                let _ = app.emit("component", event("running", done, total, None));
            }
        });
        if let Some(state) = app.try_state::<AppState>()
            && let Ok(mut installs) = state.installs.lock()
        {
            installs.remove(&id);
        }
        let total = component.size();
        let finished = match outcome {
            Ok(()) => event("done", total, total, None),
            Err(components::InstallError::Cancelled) => event("cancelled", 0, total, None),
            Err(error) => event("failed", 0, total, Some(error.message().to_string())),
        };
        let _ = app.emit("component", finished);
    });
    Ok(())
}

#[tauri::command]
fn cancel_component(id: String, state: State<'_, AppState>) -> bool {
    state
        .installs
        .lock()
        .ok()
        .and_then(|installs| installs.get(&id).cloned())
        .map(|cancel| cancel.store(true, Ordering::Relaxed))
        .is_some()
}

/// Fecha actual en UTC con el formato que exige EPUB (`AAAA-MM-DDThh:mm:ssZ`).
fn utc_now() -> String {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let (days, rest) = (seconds.div_euclid(86_400), seconds.rem_euclid(86_400));
    // Algoritmo civil de Howard Hinnant para pasar días a fecha.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rest / 3_600,
        rest % 3_600 / 60,
        rest % 60
    )
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let folder = app.path().app_data_dir()?;
            std::fs::create_dir_all(&folder)?;
            let store = Store::open(&folder.join("workspace.sqlite3"))?;
            // Restos de un cierre a mitad: se ordenan y se conservan los puntos de control.
            conversion::recover_work(&folder);
            app.manage(AppState {
                store: Mutex::new(store),
                data: folder,
                conversions: Conversions::default(),
                installs: Mutex::new(HashMap::new()),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_documents,
            open_document,
            remove_document,
            render_page,
            convert_document,
            cancel_conversion,
            set_visual_full,
            epub_chapter,
            epub_set_text,
            epub_save_as,
            epub_discard,
            page_markdown,
            save_page,
            open_in_epub_editor,
            preview_markdown,
            reveal_path,
            reveal_data,
            review_list,
            review_block_text,
            review_decide,
            result_pages,
            translation_options,
            translate_document,
            cancel_translation,
            translation_doubts,
            export_markdown,
            export_epub,
            list_components,
            install_component,
            cancel_component
        ])
        .run(tauri::generate_context!())
        .expect("no se pudo iniciar Liblevo");
}

#[cfg(test)]
mod tests {
    use super::utc_now;

    #[test]
    fn utc_now_has_epub_format() {
        let now = utc_now();
        assert_eq!(now.len(), 20);
        assert!(now.ends_with('Z') && now.as_bytes()[10] == b'T');
        assert!(now.starts_with("20"));
    }
}
