// Exportar: qué texto, qué páginas y en qué formato.
import { save } from "@tauri-apps/plugin-dialog";
import { api } from "./api";
import { $, errorToast, toast } from "./dom";
import { plural, state, stem } from "./state";

let someRange = false;

export function prepareExport(): void {
  const doc = state.current;
  if (!doc) return;
  const source = $<HTMLSelectElement>("export-source");
  const previous = source.value;
  source.replaceChildren(
    new Option("Resultado", ""),
    ...doc.translations.map((t) => new Option(`Traducción · ${t.label}`, t.language)),
  );
  const wanted = state.viewing ?? previous;
  source.value = [...source.options].some((o) => o.value === wanted) ? wanted : "";
  source.disabled = doc.translations.length === 0;
  $("range").hidden = !doc.paged;
  $<HTMLInputElement>("range-to").placeholder = String(doc.pages);
  $("epub-cover-option").hidden = !doc.paged;
  syncDefaults();
}

function setRange(on: boolean): void {
  someRange = on;
  for (const option of document.querySelectorAll<HTMLButtonElement>("#range-mode [role=radio]")) {
    option.setAttribute("aria-checked", String((option.dataset.range === "some") === on));
  }
  $("range-inputs").hidden = !on;
  if (on) $("range-from").focus();
  syncSummary();
}

function syncDefaults(): void {
  const doc = state.current;
  if (!doc) return;
  const source = $<HTMLSelectElement>("export-source").value || null;
  $<HTMLInputElement>("epub-title").value = stem(doc) + (source ? ` (${source})` : "");
  const language = $<HTMLSelectElement>("epub-language");
  if (source && [...language.options].some((o) => o.value === source)) {
    language.value = source;
  } else {
    try {
      const saved = localStorage.getItem("liblevo.epubLanguage");
      if (saved && [...language.options].some((o) => o.value === saved)) language.value = saved;
    } catch {
      // Sin almacenamiento local se queda el idioma por defecto.
    }
  }
  const translation = doc.translations.find((t) => t.language === source);
  let warning = "";
  if (!source && doc.review_pending > 0) {
    warning = `Quedan ${plural(doc.review_pending, "aviso", "avisos")} de revisión sin decidir. Puedes exportar igualmente.`;
  } else if (translation && translation.doubts > 0) {
    warning = `Esta traducción tiene ${plural(translation.doubts, "bloque", "bloques")} con avisos. Puedes exportar igualmente.`;
  }
  $("export-warning-text").textContent = warning;
  $("export-warning").hidden = !warning;
  syncSummary();
}

/** Páginas que se exportan; `null` es el documento entero. */
function range(): [number, number] | null | "invalid" {
  const doc = state.current;
  if (!doc?.paged || !someRange) return null;
  const from = $<HTMLInputElement>("range-from").value.trim();
  const to = $<HTMLInputElement>("range-to").value.trim();
  if (!from && !to) return null;
  const first = Number(from || "1");
  const last = Number(to || String(doc.pages));
  if (!Number.isInteger(first) || !Number.isInteger(last) || first < 1 || last < first) return "invalid";
  return [first, Math.min(last, doc.pages)];
}

function fileName(): string {
  const doc = state.current;
  if (!doc) return "documento";
  const source = $<HTMLSelectElement>("export-source").value;
  const pages = range();
  const suffix = Array.isArray(pages) ? ` (págs. ${pages[0]}-${pages[1]})` : "";
  return `${stem(doc)}${source ? ` (${source})` : ""}${suffix}`;
}

function syncSummary(): void {
  const doc = state.current;
  if (!doc) return;
  const source = $<HTMLSelectElement>("export-source");
  const pages = range();
  const translation = doc.translations.find((t) => t.language === source.value);
  const what = translation ? `la traducción al ${translation.label.toLowerCase()}` : "el resultado de la conversión";
  const which =
    pages === "invalid"
      ? "un intervalo de páginas no válido"
      : Array.isArray(pages)
        ? `las páginas ${pages[0]} a ${pages[1]}`
        : doc.paged
          ? `las ${plural(doc.pages, "página", "páginas")}`
          : "el documento entero";
  const summary = $("export-summary");
  summary.replaceChildren("Se exportará ");
  const strong = document.createElement("strong");
  strong.textContent = what;
  summary.append(strong, `, ${which}.`);
  $("md-name").textContent = `${fileName()}.md`;
  $("epub-name").textContent = `${fileName()}.epub`;
}

function reveal(path: string): { label: string; run: () => void } {
  return { label: "Mostrar en la carpeta", run: () => void api.revealPath(path) };
}

async function exportMarkdown(): Promise<void> {
  const doc = state.current;
  if (!doc) return;
  const pages = range();
  if (pages === "invalid") {
    toast("Revisa las páginas: la primera no puede ser mayor que la última.", "warning");
    return;
  }
  const path = await save({ defaultPath: `${fileName()}.md`, filters: [{ name: "Markdown", extensions: ["md"] }] });
  if (!path) return;
  try {
    await api.exportMarkdown(doc.id, $<HTMLSelectElement>("export-source").value || null, pages, path);
    toast("Markdown exportado.", "success", reveal(path));
  } catch (error) {
    errorToast(error);
  }
}

async function exportEpub(event: SubmitEvent): Promise<void> {
  event.preventDefault();
  const doc = state.current;
  if (!doc) return;
  const title = $<HTMLInputElement>("epub-title").value.trim();
  if (!title) {
    toast("El EPUB necesita un título.", "warning");
    $("epub-title").focus();
    return;
  }
  const pages = range();
  if (pages === "invalid") {
    toast("Revisa las páginas: la primera no puede ser mayor que la última.", "warning");
    return;
  }
  const path = await save({ defaultPath: `${fileName()}.epub`, filters: [{ name: "EPUB", extensions: ["epub"] }] });
  if (!path) return;
  try {
    await api.exportEpub({
      id: doc.id,
      translation: $<HTMLSelectElement>("export-source").value || null,
      pages,
      cover: doc.paged && $<HTMLInputElement>("epub-cover").checked,
      path,
      title,
      author: $<HTMLInputElement>("epub-author").value.trim() || null,
      language: $<HTMLSelectElement>("epub-language").value,
    });
    if ($<HTMLInputElement>("epub-open").checked) {
      const editor = await api.openInEpubEditor(path);
      toast(`EPUB exportado y abierto en ${editor}.`, "success", reveal(path));
    } else {
      toast("EPUB exportado.", "success", reveal(path));
    }
  } catch (error) {
    errorToast(error);
  }
}

export function initExport(): void {
  $("export-source").addEventListener("change", syncDefaults);
  for (const option of document.querySelectorAll<HTMLButtonElement>("#range-mode [role=radio]")) {
    option.addEventListener("click", () => setRange(option.dataset.range === "some"));
  }
  $("range-from").addEventListener("input", syncSummary);
  $("range-to").addEventListener("input", syncSummary);
  $("epub-language").addEventListener("change", (event) => {
    if ($<HTMLSelectElement>("export-source").value) return;
    try {
      localStorage.setItem("liblevo.epubLanguage", (event.target as HTMLSelectElement).value);
    } catch {
      // Recordarlo es solo una comodidad.
    }
  });
  $("export-md").addEventListener("click", () => void exportMarkdown());
  $("epub-form").addEventListener("submit", (event) => void exportEpub(event as SubmitEvent));
}
