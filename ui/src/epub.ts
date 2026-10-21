// EPUB original: edición en su sitio de los bloques de texto sin formato interno.
import { save } from "@tauri-apps/plugin-dialog";
import { api, type EpubBlock, type EpubChapter } from "./api";
import { $, button, confirmAction, errorToast, h, icon, toast } from "./dom";
import { isEpub, plural, state, stem } from "./state";

let chapter = 0;
let chapters = 0;
let edited = false;
/** Bloque en edición: su índice, el texto de partida y el campo. */
let editing: { index: number; original: string; area: HTMLTextAreaElement } | null = null;

const KINDS: Record<string, string> = {
  p: "Párrafo",
  h1: "Título",
  h2: "Título",
  h3: "Subtítulo",
  h4: "Subtítulo",
  h5: "Subtítulo",
  h6: "Subtítulo",
  li: "Elemento de lista",
  dt: "Término",
  dd: "Definición",
  td: "Celda de tabla",
  th: "Encabezado de tabla",
  figcaption: "Pie de figura",
  caption: "Título de tabla",
};

function kindOf(tag: string): string {
  return KINDS[tag.toLowerCase()] ?? "Bloque";
}

function dirty(): boolean {
  return editing !== null && editing.area.value.trim() !== editing.original.trim();
}

/** Pregunta antes de perder un cambio sin guardar. */
async function leaveEdit(): Promise<boolean> {
  if (!dirty()) return true;
  return confirmAction("¿Descartar el cambio sin guardar?", "El texto que estabas editando volverá a como estaba.", "Descartar");
}

function renderBar(): void {
  const select = $<HTMLSelectElement>("epub-chapter");
  if (select.options.length !== chapters) {
    select.replaceChildren(
      ...Array.from({ length: chapters }, (_, i) => h("option", { value: i, text: `Capítulo ${i + 1} de ${chapters}` })),
    );
  }
  select.value = String(chapter);
  $<HTMLButtonElement>("epub-prev").disabled = chapter <= 0;
  $<HTMLButtonElement>("epub-next").disabled = chapter + 1 >= chapters;
  const status = $("epub-status");
  status.className = edited ? "pill warn" : "pill muted";
  status.replaceChildren(icon(edited ? "edit" : "check"), edited ? "Con ediciones" : "Sin ediciones");
  status.title = edited
    ? "Hay cambios en la copia de trabajo. Guárdalos como EPUB nuevo o descártalos."
    : "La copia de trabajo es igual que el EPUB original.";
  $<HTMLButtonElement>("epub-discard").disabled = !edited;
}

function blockItem(block: EpubBlock): HTMLLIElement {
  const level = /^h([1-6])$/i.exec(block.tag)?.[1];
  const style = !level ? "" : Number(level) <= 2 ? " heading" : " subheading";
  const text = h("p", { class: `epub-text${style}`, text: block.text });
  const kind = h("span", { class: "epub-kind", text: kindOf(block.tag) });
  const item = h("li", { class: block.editable ? "epub-block" : "epub-block locked", "data-index": block.index }, kind, text);
  if (block.editable) {
    item.append(
      button(icon("edit"), () => void startEdit(block, item), {
        class: "icon-button epub-edit",
        "aria-label": `Editar: ${block.text.slice(0, 60)}`,
        "data-tip": "Editar (o doble clic)",
        "data-tip-align": "end",
      }),
    );
    text.addEventListener("dblclick", () => void startEdit(block, item));
  } else {
    item.append(
      h(
        "span",
        { class: "epub-locked", title: "Tiene cursivas, enlaces u otro formato dentro. Edítalo en Sigil o Calibre." },
        "Con formato",
      ),
    );
  }
  return item;
}

function render(view: EpubChapter): void {
  chapter = view.chapter;
  chapters = view.chapters;
  edited = view.edited;
  editing = null;
  renderBar();
  const editable = view.blocks.filter((b) => b.editable).length;
  $("epub-count").textContent = view.blocks.length
    ? `${plural(view.blocks.length, "bloque", "bloques")} · ${plural(editable, "editable", "editables")}`
    : "";
  const list = $("epub-blocks");
  list.replaceChildren(...view.blocks.map(blockItem));
  list.hidden = view.blocks.length === 0;
  $("epub-empty").hidden = view.blocks.length > 0;
}

async function startEdit(block: EpubBlock, item: HTMLLIElement): Promise<void> {
  if (editing?.index === block.index) return editing.area.focus();
  if (!(await leaveEdit())) return editing?.area.focus();
  if (editing) await load(chapter, true);
  const target = $("epub-blocks").querySelector<HTMLLIElement>(`[data-index="${block.index}"]`) ?? item;
  const area = h("textarea", { class: "epub-area", "aria-label": `Texto: ${kindOf(block.tag)}` });
  area.value = block.text;
  const grow = (): void => {
    area.style.height = "auto";
    area.style.height = `${area.scrollHeight + 2}px`;
  };
  const saveButton = button("Guardar", () => void saveEdit(), { class: "primary small", disabled: true });
  area.addEventListener("input", () => {
    grow();
    saveButton.disabled = !dirty();
  });
  area.addEventListener("keydown", (event) => {
    if ((event.ctrlKey || event.metaKey) && (event.key.toLowerCase() === "s" || event.key === "Enter")) {
      event.preventDefault();
      event.stopPropagation();
      void saveEdit();
    } else if (event.key === "Escape") {
      event.preventDefault();
      event.stopPropagation();
      void cancelEdit();
    }
  });
  const actions = h(
    "div",
    { class: "epub-edit-actions" },
    h("span", { class: "hint" }, h("kbd", { text: "Ctrl" }), "+", h("kbd", { text: "S" }), " guarda · ", h("kbd", { text: "Esc" }), " cancela"),
    button("Cancelar", () => void cancelEdit(), { class: "ghost small" }),
    saveButton,
  );
  target.classList.add("editing");
  target.querySelector(".epub-text")?.replaceWith(area);
  target.querySelector(".epub-edit")?.remove();
  target.append(actions);
  editing = { index: block.index, original: block.text, area };
  grow();
  area.focus();
  area.setSelectionRange(area.value.length, area.value.length);
}

async function cancelEdit(): Promise<void> {
  if (!editing || !(await leaveEdit())) return;
  const index = editing.index;
  await load(chapter, true);
  focusBlock(index);
}

async function saveEdit(): Promise<void> {
  const doc = state.current;
  if (!doc || !editing) return;
  const { index, area } = editing;
  if (!area.value.trim()) {
    toast("El bloque no puede quedar vacío. Para quitarlo, usa Sigil o Calibre.", "warning");
    return;
  }
  if (!dirty()) {
    editing = null;
    await load(chapter, true);
    focusBlock(index);
    return;
  }
  try {
    render(await api.epubSetText(doc.id, chapter, index, area.value));
    focusBlock(index, true);
    toast("Cambio guardado en la copia de trabajo del EPUB.", "success");
  } catch (error) {
    errorToast(error);
  }
}

/** Devuelve el foco al bloque y, si se acaba de guardar, lo resalta un momento. */
function focusBlock(index: number, flash = false): void {
  const item = $("epub-blocks").querySelector<HTMLElement>(`[data-index="${index}"]`);
  if (!item) return;
  (item.querySelector<HTMLElement>(".epub-edit") ?? item).focus({ preventScroll: true });
  item.scrollIntoView({ block: "nearest" });
  if (flash) {
    item.classList.add("saved");
    setTimeout(() => item.classList.remove("saved"), 1400);
  }
}

export async function load(target = chapter, keepScroll = false): Promise<void> {
  const doc = state.current;
  if (!doc || !isEpub(doc)) return;
  const panel = document.querySelector<HTMLElement>('[data-panel="epub"]');
  const scroll = panel?.scrollTop ?? 0;
  $("epub-blocks").setAttribute("aria-busy", "true");
  try {
    render(await api.epubChapter(doc.id, target));
    if (panel) panel.scrollTop = keepScroll ? scroll : 0;
  } catch (error) {
    errorToast(error);
  } finally {
    $("epub-blocks").removeAttribute("aria-busy");
  }
}

async function go(target: number): Promise<void> {
  if (target < 0 || target >= chapters || target === chapter) return;
  if (!(await leaveEdit())) {
    $<HTMLSelectElement>("epub-chapter").value = String(chapter);
    return;
  }
  await load(target);
}

export function resetEpub(): void {
  chapter = 0;
  chapters = 0;
  edited = false;
  editing = null;
  $<HTMLSelectElement>("epub-chapter").replaceChildren();
}

/** ←/→ y AvPág/RePág cambian de capítulo cuando no se está escribiendo. */
export function epubKey(event: KeyboardEvent): boolean {
  if (editing) return false;
  switch (event.key) {
    case "ArrowLeft":
    case "PageUp":
      void go(chapter - 1);
      return true;
    case "ArrowRight":
    case "PageDown":
      void go(chapter + 1);
      return true;
    case "Home":
      void go(0);
      return true;
    case "End":
      void go(chapters - 1);
      return true;
  }
  return false;
}

export function initEpub(): void {
  $("epub-prev").addEventListener("click", () => void go(chapter - 1));
  $("epub-next").addEventListener("click", () => void go(chapter + 1));
  $<HTMLSelectElement>("epub-chapter").addEventListener("change", (event) => {
    void go(Number((event.target as HTMLSelectElement).value));
  });
  $("epub-empty-next").addEventListener("click", () => void go(chapter + 1));
  $("epub-save").addEventListener("click", async () => {
    const doc = state.current;
    if (!doc || !(await leaveEdit())) return;
    const path = await save({ defaultPath: `${stem(doc)}-editado.epub`, filters: [{ name: "EPUB", extensions: ["epub"] }] });
    if (!path) return;
    try {
      await api.epubSaveAs(doc.id, path);
      toast(edited ? "EPUB editado guardado." : "No había ediciones: se ha guardado una copia del EPUB original.", "success", {
        label: "Mostrar en la carpeta",
        run: () => void api.revealPath(path),
      });
    } catch (error) {
      errorToast(error);
    }
  });
  $("epub-discard").addEventListener("click", async () => {
    const doc = state.current;
    if (!doc) return;
    const ok = await confirmAction(
      "¿Descartar todas las ediciones del EPUB?",
      "Se perderán los cambios y las traducciones hechos en la copia de trabajo, en todos los capítulos. El EPUB original no se toca.",
      "Descartar",
    );
    if (!ok) return;
    try {
      await api.epubDiscard(doc.id);
      editing = null;
      await load(chapter, true);
      toast("Ediciones del EPUB descartadas.");
    } catch (error) {
      errorToast(error);
    }
  });
}
