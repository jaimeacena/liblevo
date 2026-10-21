// Liblevo · arranque de la interfaz: navegación, cabecera del documento, atajos y eventos del motor.
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { open } from "@tauri-apps/plugin-dialog";
import {
  api,
  type ComponentEvent,
  type ConversionEvent,
  type DocumentInfo,
  type Summary,
  type TranslationEvent,
} from "./api";
import { $, confirmAction, errorToast, hydrateIcons, toast } from "./dom";
import { epubKey, initEpub, load as loadEpub, resetEpub } from "./epub";
import { initExport, prepareExport } from "./exporter";
import { focusSearch, forgetThumb, initLibrary, libraryKey, renderLibrary } from "./library";
import { initPalette, openPalette } from "./palette";
import {
  checkComponents,
  downloadText,
  initPanels,
  openComponents,
  renderActivity,
  renderComponents,
  toggleActivity,
} from "./panels";
import { editorKey, initRead, isEditing, readKey, reloadText, renderPage, resetRead, showPage, syncControls } from "./read";
import { focusItem, initReview, loadReview, resetReview, reviewEditing, reviewKey } from "./review";
import {
  actions,
  duration,
  extension,
  formatOf,
  isEpub,
  nextStep,
  plural,
  sizeLabel,
  state,
  stateLabel,
  stem,
  SUPPORTED,
  type Tab,
} from "./state";
import { initTranslate, prepareTranslate, renderProgress, renderTranslations } from "./translate";

const TABS: Tab[] = ["leer", "revisar", "traducir", "exportar", "epub"];

// --- Navegación -------------------------------------------------------------------------------

function showView(view: "library" | "doc"): void {
  state.view = view;
  $("view-library").hidden = view !== "library";
  $("view-doc").hidden = view !== "doc";
  $("nav-library").setAttribute("aria-current", view === "library" ? "page" : "false");
  if (view === "library") renderLibrary();
}

function tabAvailable(tab: Tab, doc: DocumentInfo): boolean {
  switch (tab) {
    case "leer":
      return true;
    case "revisar":
    case "exportar":
      return doc.converted;
    case "traducir":
      return doc.converted || isEpub(doc);
    case "epub":
      return isEpub(doc);
  }
}

function setTab(tab: Tab): void {
  const doc = state.current;
  if (doc && !tabAvailable(tab, doc)) tab = "leer";
  state.tab = tab;
  for (const button of document.querySelectorAll<HTMLButtonElement>(".tabs [role=tab]")) {
    const on = button.dataset.tab === tab;
    button.setAttribute("aria-selected", String(on));
    button.tabIndex = on ? 0 : -1;
  }
  for (const panel of document.querySelectorAll<HTMLElement>("[data-panel]")) {
    panel.hidden = panel.dataset.panel !== tab;
  }
  if (doc) renderHeader();
  if (tab === "leer") {
    syncControls();
    void renderPage();
  }
  if (tab === "revisar") void loadReview(true);
  if (tab === "traducir") void prepareTranslate();
  if (tab === "exportar") prepareExport();
  if (tab === "epub") void loadEpub();
}

async function openDoc(doc: DocumentInfo, page = 1): Promise<void> {
  const changed = state.current?.id !== doc.id;
  state.current = doc;
  state.selectedId = doc.id;
  if (changed) {
    state.viewing = null;
    state.page = page;
    resetRead();
    resetReview();
    resetEpub();
    $("translation-review").hidden = true;
  }
  showView("doc");
  renderHeader();
  const tab = changed ? "leer" : state.tab;
  setTab(tab);
  if (tab === "leer" && !changed && page !== state.page) await showPage(page);
}

async function openTab(doc: DocumentInfo, tab: Tab): Promise<void> {
  await openDoc(doc);
  setTab(tab);
}

// --- Cabecera ---------------------------------------------------------------------------------

function renderHeader(): void {
  const doc = state.current;
  if (!doc) return;
  const format = formatOf(doc.name);
  $("doc-format").className = `badge ${format}`;
  $("doc-format").textContent = format.toUpperCase();
  $("doc-name").textContent = stem(doc);
  $("doc-name").title = doc.path;
  $("doc-meta").textContent = `${sizeLabel(doc)} · ${stateLabel(doc)}`;
  $("doc-missing").hidden = doc.available;

  const primary = $<HTMLButtonElement>("doc-primary");
  const step = nextStep(doc);
  // No se repite la pestaña en la que ya se está.
  const target: Partial<Record<string, Tab>> = { review: "revisar", export: "exportar" };
  primary.hidden = step === "none" || target[step] === state.tab;
  primary.className = step === "cancel" ? "danger" : "primary";
  primary.textContent = {
    convert: "Convertir",
    resume: "Reanudar conversión",
    cancel: "Cancelar conversión",
    review: `Revisar ${plural(doc.review_pending, "aviso", "avisos")}`,
    export: "Exportar",
    none: "",
  }[step];
  primary.dataset.step = step;

  $("menu-reconvert").hidden = doc.converting || (!doc.converted && !doc.resumable) || !doc.available;
  $("menu-visual").hidden = !doc.paged;
  $("menu-reveal").hidden = !doc.available;
  $<HTMLInputElement>("visual-full").checked = doc.visual_full;
  $<HTMLInputElement>("visual-full").disabled = doc.converting;

  // Cada pestaña muestra el estado de su paso: punto de color y un detalle breve.
  const set = (step: string, cls: string, detail: string) => {
    const tab = document.querySelector(`.tabs [data-step="${step}"]`) as HTMLButtonElement;
    tab.classList.remove("done", "active", "attention");
    if (cls) tab.classList.add(cls);
    const note = tab.querySelector(".tdetail");
    if (note) note.textContent = detail;
  };
  set(
    "convert",
    doc.converting ? "active" : doc.converted ? "done" : doc.resumable ? "attention" : "",
    doc.converting ? "convirtiendo" : doc.converted ? "" : doc.resumable ? "interrumpida" : "sin convertir",
  );
  set(
    "review",
    !doc.converted ? "" : doc.review_pending > 0 ? "attention" : doc.review_total === null ? "" : "done",
    doc.converted && doc.review_pending > 0 ? doc.review_pending.toLocaleString("es-ES") : "",
  );
  const [done, total] = state.translationProgress.get(doc.id) ?? [0, 0];
  set(
    "translate",
    doc.translating ? "active" : doc.translations.length ? "done" : "",
    doc.translating
      ? total
        ? `${Math.round((100 * done) / total)} %`
        : "…"
      : doc.translations.map((t) => t.language.toUpperCase()).join(" "),
  );
  set("export", "", "");
  for (const button of document.querySelectorAll<HTMLButtonElement>(".tabs [role=tab]")) {
    const tab = button.dataset.tab as Tab;
    button.hidden = tab === "epub" && !isEpub(doc);
    button.disabled = !tabAvailable(tab, doc);
  }
  if (!tabAvailable(state.tab, doc)) setTab("leer");
}

// --- Acciones ---------------------------------------------------------------------------------

async function refresh(selectId?: string): Promise<void> {
  try {
    state.documents = await api.listDocuments();
  } catch (error) {
    errorToast(error);
  }
  const keep = state.documents.find((d) => d.id === (selectId ?? state.current?.id));
  state.current = keep ?? null;
  if (state.view === "doc" && !state.current) showView("library");
  if (state.view === "library") renderLibrary();
  if (state.current && state.view === "doc") {
    renderHeader();
    if (state.tab === "traducir") {
      renderProgress();
      renderTranslations();
    }
  }
  renderActivity();
}

async function openPaths(paths: string[]): Promise<void> {
  const usable = paths.filter((p) => SUPPORTED.includes(extension(p)));
  if (!usable.length) {
    toast("Formato no admitido. Liblevo abre PDF, EPUB, DOCX, HTML, TXT y Markdown.", "warning");
    return;
  }
  const added: DocumentInfo[] = [];
  for (const path of usable) {
    try {
      added.push(await api.openDocument(path));
    } catch (error) {
      errorToast(error);
    }
  }
  if (!added.length) return;
  await refresh();
  if (added.length === 1) {
    const doc = state.documents.find((d) => d.id === added[0].id);
    if (doc) await openDoc(doc);
  } else {
    state.selectedId = added[0].id;
    showView("library");
    toast(`${plural(added.length, "documento añadido", "documentos añadidos")} a la biblioteca.`, "success");
  }
}

async function addDocuments(): Promise<void> {
  const selected = await open({ multiple: true, directory: false, filters: [{ name: "Documentos", extensions: SUPPORTED }] });
  if (!selected) return;
  await openPaths(Array.isArray(selected) ? selected : [selected]);
}

async function convert(doc: DocumentInfo): Promise<void> {
  if (doc.converted) {
    const ok = await confirmAction(
      "¿Volver a convertir?",
      `Se sustituirá el resultado de «${doc.name}». Sus traducciones y las decisiones de revisión se borrarán, porque ya no corresponderán al nuevo resultado.`,
      "Volver a convertir",
    );
    if (!ok) return;
  }
  try {
    await api.convert(doc.id);
    toast(`Convirtiendo ${doc.name}. Puedes seguir trabajando; te avisaré al terminar.`);
    await refresh(state.current?.id);
  } catch (error) {
    errorToast(error);
  }
}

/** «a», «a y b», «a, b y c». */
function listText(items: string[]): string {
  return items.length > 1 ? `${items.slice(0, -1).join(", ")} y ${items[items.length - 1]}` : (items[0] ?? "");
}

async function remove(doc: DocumentInfo): Promise<boolean> {
  const lost = [
    doc.converted ? "su resultado" : "",
    doc.translations.length ? plural(doc.translations.length, "traducción", "traducciones") : "",
    doc.converted && doc.review_total ? "las decisiones de revisión" : "",
  ].filter(Boolean);
  const ok = await confirmAction(
    `¿Quitar «${stem(doc)}» de la biblioteca?`,
    (lost.length ? `Quitarlo borra ${listText(lost)}. ` : "") + "El archivo original no se toca y puedes volver a añadirlo.",
    "Quitar",
  );
  if (!ok) return false;
  try {
    await api.removeDocument(doc.id);
    forgetThumb(doc.id);
    if (state.current?.id === doc.id) {
      state.current = null;
      showView("library");
    }
    await refresh();
    toast(`«${doc.name}» se ha quitado de la biblioteca.`);
    return true;
  } catch (error) {
    errorToast(error);
    return false;
  }
}

actions.refresh = refresh;
actions.open = openDoc;
actions.openTab = openTab;
actions.showLibrary = () => showView("library");
actions.setTab = setTab;
actions.goTo = async (page, block) => {
  setTab("leer");
  await showPage(page, block);
};
actions.reviewAt = (page, block) => {
  setTab("revisar");
  void focusItem(page, block);
};
actions.openComponents = openComponents;
actions.addDocuments = addDocuments;
actions.convert = convert;
actions.remove = remove;
actions.openPalette = openPalette;

// --- Eventos del motor ------------------------------------------------------------------------

function doubtNote(summary: Summary): string {
  const reread = `${summary.reread_pages} releídas con el modelo visual`;
  if (!summary.doubtful_pages) return summary.reread_pages ? `; ${reread}` : "";
  const detail = summary.visual_available ? reread : "sin releer (falta el modelo visual)";
  return `; ${summary.doubtful_pages} páginas dudosas, ${detail}`;
}

const nameOf = (id: string) => state.documents.find((d) => d.id === id)?.name ?? "El documento";

void listen<ConversionEvent>("conversion", async (event) => {
  const { id, state: phase, message, summary } = event.payload;
  const name = nameOf(id);
  await refresh();
  const doc = state.documents.find((d) => d.id === id);
  const openIt = doc && (state.current?.id !== id || state.view !== "doc") ? { label: "Abrir", run: () => void openDoc(doc) } : undefined;
  if (phase === "done" && summary) {
    toast(
      `«${name}» convertido en ${duration(summary.seconds)}${summary.scanned ? " (escaneo leído con OCR)" : ""}: ` +
        `${plural(summary.headings, "título", "títulos")}, ${plural(summary.images, "imagen", "imágenes")}${doubtNote(summary)}.` +
        (doc && doc.review_pending ? ` ${plural(doc.review_pending, "aviso", "avisos")} para revisar.` : ""),
      "success",
      openIt,
    );
  } else if (phase === "failed") {
    toast(`No se pudo convertir «${name}»: ${message ?? "error desconocido"}`, "error");
  } else if (phase === "cancelled") {
    toast(`Conversión de «${name}» cancelada. Lo hecho se guarda para reanudarla.`, "warning");
  }
  if (state.current?.id === id && phase !== "running" && state.view === "doc") {
    resetReview();
    await reloadText();
    if (state.tab === "leer") await renderPage();
    if (state.tab === "revisar") await loadReview(true);
  }
});

void listen<TranslationEvent>("translation", async (event) => {
  const { id, state: phase, done, total, message, summary, epub } = event.payload;
  if (phase === "running") {
    state.translationProgress.set(id, [done, total]);
    if (state.view === "library") renderLibrary();
    renderActivity();
    if (state.current?.id === id && state.view === "doc") {
      renderHeader();
      if (state.tab === "traducir") renderProgress();
    }
    return;
  }
  state.translationProgress.delete(id);
  const name = nameOf(id);
  await refresh();
  const doc = state.documents.find((d) => d.id === id);
  if (phase === "done" && summary) {
    toast(
      `${epub ? "EPUB original de " : ""}«${name}» traducido al ${summary.label.toLowerCase()} en ${duration(summary.seconds)}` +
        (summary.doubts ? `: ${plural(summary.doubts, "bloque", "bloques")} con avisos` : ", sin avisos") +
        (summary.untouched ? `; ${summary.untouched} con formato interno sin traducir` : "") +
        ".",
      "success",
      doc
        ? {
            label: epub ? "Ver EPUB" : "Leer",
            run: () => {
              void openDoc(doc).then(() => {
                if (!epub) state.viewing = summary.language;
                setTab(epub ? "epub" : "leer");
              });
            },
          }
        : undefined,
    );
  } else if (phase === "failed") {
    toast(`No se pudo traducir «${name}»: ${message ?? "error desconocido"}`, "error");
  } else {
    toast(`Traducción de «${name}» cancelada. Lo traducido se guarda y se reanuda.`, "warning");
  }
  if (state.current?.id === id && state.view === "doc") {
    if (state.tab === "traducir") void prepareTranslate();
    if (state.tab === "epub" && epub) void loadEpub();
  }
});

void listen<ComponentEvent>("component", async (event) => {
  const { id, state: phase, done, total, message } = event.payload;
  state.componentProgress.set(id, total ? done / total : 0);
  if (phase === "running") {
    state.installing.add(id);
    const bar = document.querySelector<HTMLProgressElement>(`#component-list progress[data-id="${id}"]`);
    if (bar) bar.value = total ? done / total : 0;
    const text = document.querySelector<HTMLElement>(`#component-list [data-progress="${id}"]`);
    if (text) text.textContent = downloadText(id);
    renderActivity();
    return;
  }
  state.installing.delete(id);
  if (phase === "done") toast("Componente descargado, comprobado e instalado.", "success");
  else if (phase === "failed") toast(message ?? "No se pudo instalar el componente.", "error");
  else toast("Descarga cancelada; no se ha instalado nada.", "warning");
  if ($<HTMLDialogElement>("components").open) await renderComponents();
  else await checkComponents();
  renderActivity();
  if (state.tab === "traducir" && state.view === "doc") void prepareTranslate();
});

// --- Controles --------------------------------------------------------------------------------

function initDocument(): void {
  $("doc-back").addEventListener("click", () => showView("library"));
  $("nav-library").addEventListener("click", () => showView("library"));
  $("nav-home").addEventListener("click", () => showView("library"));
  const tabs = [...document.querySelectorAll<HTMLButtonElement>(".tabs [role=tab]")];
  for (const button of tabs) {
    button.addEventListener("click", () => setTab(button.dataset.tab as Tab));
    // Flechas para moverse entre pestañas, como en cualquier lista de pestañas.
    button.addEventListener("keydown", (event) => {
      if (event.key !== "ArrowRight" && event.key !== "ArrowLeft") return;
      const enabled = tabs.filter((t) => !t.disabled && !t.hidden);
      const index = enabled.indexOf(button);
      const next = enabled[(index + (event.key === "ArrowRight" ? 1 : -1) + enabled.length) % enabled.length];
      setTab(next.dataset.tab as Tab);
      next.focus();
      event.preventDefault();
      event.stopPropagation();
    });
  }
  $("doc-primary").addEventListener("click", () => {
    const doc = state.current;
    if (!doc) return;
    const step = $("doc-primary").dataset.step;
    if (step === "convert" || step === "resume") void convert(doc);
    else if (step === "cancel") void api.cancelConversion(doc.id);
    else if (step === "review") setTab("revisar");
    else if (step === "export") setTab("exportar");
  });
  const menu = $("doc-menu");
  const more = $("doc-more");
  const items = () => [...menu.querySelectorAll<HTMLElement>("button:not([hidden]), label:not([hidden])")];
  const closeMenu = (focusButton = false) => {
    menu.hidden = true;
    more.setAttribute("aria-expanded", "false");
    if (focusButton) more.focus();
  };
  more.addEventListener("click", (event) => {
    event.stopPropagation();
    menu.hidden = !menu.hidden;
    more.setAttribute("aria-expanded", String(!menu.hidden));
    if (!menu.hidden) items()[0]?.focus();
  });
  menu.addEventListener("keydown", (event) => {
    const list = items();
    const index = list.indexOf(document.activeElement as HTMLElement);
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      const next = list[(index + (event.key === "ArrowDown" ? 1 : -1) + list.length) % list.length];
      next?.focus();
      event.preventDefault();
    } else if (event.key === "Escape") {
      closeMenu(true);
      event.stopPropagation();
    }
  });
  for (const label of menu.querySelectorAll<HTMLElement>("label")) label.tabIndex = 0;
  document.addEventListener("mousedown", (event) => {
    if (!menu.hidden && !menu.contains(event.target as Node) && !more.contains(event.target as Node)) closeMenu();
  });
  $("menu-reconvert").addEventListener("click", () => {
    closeMenu();
    if (state.current) void convert(state.current);
  });
  $("menu-reveal").addEventListener("click", () => {
    closeMenu();
    if (state.current) void api.revealPath(state.current.path).catch(errorToast);
  });
  $("menu-remove").addEventListener("click", () => {
    closeMenu();
    if (state.current) void remove(state.current);
  });
  $("visual-full").addEventListener("change", async (event) => {
    const doc = state.current;
    if (!doc) return;
    const on = (event.target as HTMLInputElement).checked;
    try {
      await api.setVisualFull(doc.id, on);
      await refresh(doc.id);
      toast(
        on
          ? "La próxima conversión releerá todas las páginas del escaneo con el modelo visual (lento)."
          : "La próxima conversión releerá solo las páginas dudosas o con figuras.",
      );
    } catch (error) {
      errorToast(error);
    }
  });
}

function typing(target: EventTarget | null): boolean {
  return (
    target instanceof HTMLInputElement ||
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    (target instanceof HTMLElement && target.isContentEditable)
  );
}

document.addEventListener("keydown", (event) => {
  const key = event.key.toLowerCase();
  const ctrl = event.ctrlKey || event.metaKey;
  if (document.querySelector("dialog[open]")) return;
  if (ctrl && key === "k") {
    event.preventDefault();
    openPalette();
    return;
  }
  if (ctrl && key === "o") {
    event.preventDefault();
    void addDocuments();
    return;
  }
  if (event.key === "Escape" && !$("activity").hidden) {
    toggleActivity(false);
    return;
  }
  // Las ediciones aceptan Ctrl+S y Esc aunque el foco esté en el campo de texto.
  if (state.view === "doc" && state.tab === "leer" && isEditing() && editorKey(event)) {
    event.preventDefault();
    return;
  }
  if (state.view === "doc" && state.tab === "revisar" && reviewEditing() && reviewKey(event)) {
    event.preventDefault();
    return;
  }
  if (ctrl && key === "f" && state.view === "library") {
    event.preventDefault();
    focusSearch();
    return;
  }
  if (ctrl && /^[1-5]$/.test(event.key) && state.view === "doc") {
    event.preventDefault();
    setTab(TABS[Number(event.key) - 1]);
    return;
  }
  if (event.altKey && event.key === "ArrowLeft" && state.view === "doc") {
    event.preventDefault();
    showView("library");
    return;
  }
  if (ctrl || event.altKey || typing(event.target)) return;
  if ((event.target as HTMLElement | null)?.closest?.('[role="tablist"], [role="menu"]')) return;
  let handled = false;
  if (state.view === "library") handled = libraryKey(event);
  else if (state.tab === "leer") handled = readKey(event);
  else if (state.tab === "revisar") handled = reviewKey(event);
  else if (state.tab === "epub") handled = epubKey(event);
  if (handled) event.preventDefault();
});

// Arrastrar y soltar documentos sobre la ventana.
void getCurrentWebview().onDragDropEvent((event) => {
  const kind = event.payload.type;
  $("drop-overlay").hidden = !(kind === "enter" || kind === "over");
  if (kind === "drop") void openPaths(event.payload.paths);
});

// --- Arranque ---------------------------------------------------------------------------------

hydrateIcons();
initPanels();
initPalette();
initLibrary();
initDocument();
initRead();
initReview();
initTranslate();
initExport();
initEpub();
void refresh();
void checkComponents();
