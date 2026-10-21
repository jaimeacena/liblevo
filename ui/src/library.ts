// Biblioteca: tabla de documentos con orden, filtros, selección e inspector.
import { api, type DocumentInfo } from "./api";
import { $, button, frag, h, icon } from "./dom";
import { actions, formatOf, isEpub, nextStep, plural, state, stateLabel, type Tab } from "./state";

type Filter = "all" | "pending" | "review" | "translated" | "busy";
type Sort = "name" | "pages" | "state" | "review";

let filter: Filter = "all";
let query = "";
let sort: Sort = "name";
let ascending = true;
const thumbs = new Map<string, string>();

try {
  const saved = JSON.parse(localStorage.getItem("liblevo.sort") ?? "null") as { sort?: Sort; ascending?: boolean } | null;
  if (saved?.sort) {
    sort = saved.sort;
    ascending = saved.ascending ?? true;
  }
} catch {
  // Sin almacenamiento local se ordena por nombre.
}

const FILTERS: Record<Filter, (doc: DocumentInfo) => boolean> = {
  all: () => true,
  pending: (doc) => !doc.converted || doc.resumable,
  review: (doc) => doc.converted && doc.review_pending > 0,
  translated: (doc) => doc.translations.length > 0,
  busy: (doc) => doc.converting || doc.translating,
};

const normal = (s: string) => s.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();

function stateRank(doc: DocumentInfo): number {
  if (doc.converting || doc.translating) return 0;
  if (doc.resumable) return 1;
  if (!doc.converted) return 2;
  return 3;
}

function visibleDocs(): DocumentInfo[] {
  const docs = state.documents.filter((doc) => FILTERS[filter](doc) && (!query || normal(doc.name).includes(normal(query))));
  const compare: Record<Sort, (a: DocumentInfo, b: DocumentInfo) => number> = {
    name: (a, b) => a.name.localeCompare(b.name, "es", { numeric: true, sensitivity: "base" }),
    pages: (a, b) => a.pages - b.pages,
    state: (a, b) => stateRank(a) - stateRank(b),
    review: (a, b) => a.review_pending - b.review_pending,
  };
  return docs.sort((a, b) => (ascending ? 1 : -1) * compare[sort](a, b) || compare.name(a, b));
}

function statusCell(doc: DocumentInfo): HTMLElement {
  const cell = h("div", { class: "cell" });
  if (!doc.available) cell.append(h("span", { class: "pill muted", text: "Sin original" }));
  if (doc.converting || doc.translating) cell.append(h("span", { class: "pill busy", text: stateLabel(doc) }));
  else if (doc.resumable) cell.append(h("span", { class: "pill warn", text: "Interrumpida" }));
  else if (!doc.converted) cell.append(h("span", { class: "pill muted", text: "Sin convertir" }));
  else cell.append(h("span", { class: "pill ok" }, icon("check"), "Convertido"));
  return cell;
}

function reviewCell(doc: DocumentInfo): HTMLElement {
  const cell = h("div", { class: "cell" });
  if (!doc.converted) cell.append(h("span", { class: "muted-dash", text: "—" }));
  else if (doc.review_pending > 0) cell.append(h("span", { class: "pill warn", text: plural(doc.review_pending, "aviso", "avisos") }));
  else if (doc.review_total === null) cell.append(h("span", { class: "subtle", text: "Por calcular" }));
  else cell.append(h("span", { class: "pill ok" }, icon("check"), doc.review_total ? "Revisado" : "Sin avisos"));
  return cell;
}

function translationCell(doc: DocumentInfo): HTMLElement {
  const cell = h("div", { class: "cell" });
  if (!doc.translations.length) cell.append(h("span", { class: "muted-dash", text: "—" }));
  for (const t of doc.translations.slice(0, 3)) {
    cell.append(h("span", { class: "lang", title: `Traducción al ${t.label.toLowerCase()}`, text: t.language.toUpperCase() }));
  }
  return cell;
}

function select(id: string, focus = false): void {
  state.selectedId = id;
  for (const row of document.querySelectorAll<HTMLElement>(".doc-row")) {
    const on = row.dataset.id === id;
    row.setAttribute("aria-selected", String(on));
    row.tabIndex = on ? 0 : -1;
    if (on && focus) {
      row.focus();
      row.scrollIntoView({ block: "nearest" });
    }
  }
  renderInspector();
}

function row(doc: DocumentInfo): HTMLLIElement {
  const format = formatOf(doc.name);
  const item = h(
    "li",
    {
      class: `doc-row${doc.available ? "" : " missing"}`,
      role: "row",
      "data-id": doc.id,
      "aria-selected": String(doc.id === state.selectedId),
      tabindex: doc.id === state.selectedId ? 0 : -1,
    },
    h(
      "div",
      { class: "doc-cell-name", role: "gridcell" },
      h("span", { class: `badge ${format}`, text: format.toUpperCase() }),
      h("div", { class: "titles" }, h("span", { class: "name", title: doc.name, text: doc.name }), h("span", { class: "sub", text: doc.path })),
    ),
    h("span", { class: "num", role: "gridcell", text: doc.paged ? doc.pages.toLocaleString("es-ES") : "—" }),
    statusCell(doc),
    reviewCell(doc),
    translationCell(doc),
    h(
      "div",
      { class: "row-actions", role: "gridcell" },
      button(icon("right"), (event) => {
        event.stopPropagation();
        void actions.open(doc);
      }, { class: "icon-button", "aria-label": `Abrir ${doc.name}`, title: "Abrir (Enter)" }),
    ),
  );
  if (doc.converting || doc.translating) {
    const bar = h("progress", { max: 1 });
    const [done, total] = state.translationProgress.get(doc.id) ?? [0, 0];
    if (doc.translating && total) bar.value = done / total;
    item.append(bar);
  }
  item.addEventListener("click", () => select(doc.id));
  item.addEventListener("dblclick", () => void actions.open(doc));
  return item;
}

// --- Inspector --------------------------------------------------------------------------------

async function loadThumb(doc: DocumentInfo, target: HTMLElement): Promise<void> {
  if (!doc.paged || !doc.available) return;
  let url = thumbs.get(doc.id);
  if (!url) {
    try {
      const bytes = await api.renderPage(doc.id, 1, Math.round(300 * window.devicePixelRatio));
      url = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
      thumbs.set(doc.id, url);
    } catch {
      return;
    }
  }
  if (state.selectedId === doc.id) target.replaceChildren(h("img", { src: url, alt: `Primera página de ${doc.name}` }));
}

function fact(label: string, ...value: (Node | string)[]): HTMLDivElement {
  return h("div", {}, h("dt", { text: label }), h("dd", {}, ...value));
}

function renderInspector(): void {
  const panel = $("inspector");
  const doc = state.documents.find((d) => d.id === state.selectedId);
  if (!doc) {
    panel.replaceChildren(
      h(
        "div",
        { class: "inspector-empty" },
        icon("page"),
        h("p", { text: "Selecciona un documento para ver su resumen." }),
        h(
          "dl",
          {},
          h("dt", {}, h("kbd", { text: "↑" }), " ", h("kbd", { text: "↓" })),
          h("dd", { text: "moverse" }),
          h("dt", {}, h("kbd", { text: "Enter" })),
          h("dd", { text: "abrir" }),
          h("dt", {}, h("kbd", { text: "Supr" })),
          h("dd", { text: "quitar" }),
        ),
      ),
    );
    return;
  }
  const format = formatOf(doc.name);
  const thumb = h(
    "div",
    { class: "thumb" },
    h("div", { class: `format-tile ${format}`, text: format.toUpperCase() }),
  );
  void loadThumb(doc, thumb);
  const step = nextStep(doc);
  const primaryLabel: Record<string, string> = {
    convert: "Convertir",
    resume: "Reanudar conversión",
    cancel: "Abrir",
    review: `Revisar ${plural(doc.review_pending, "aviso", "avisos")}`,
    export: "Abrir",
    none: "Abrir",
  };
  const primary = button(primaryLabel[step], () => {
    if (step === "convert" || step === "resume") void actions.convert(doc);
    else if (step === "review") void actions.openTab(doc, "revisar");
    else void actions.open(doc);
  }, { class: "primary" });
  const quick = (label: string, iconName: string, tab: Tab, enabled: boolean) =>
    button(frag(icon(iconName), label), () => void actions.openTab(doc, tab), { disabled: !enabled });
  const translations = doc.translations.length
    ? doc.translations.map((t) => h("span", { class: "lang", text: t.language.toUpperCase() }))
    : ["—"];
  panel.replaceChildren(
    thumb,
    h("h2", { text: doc.name }),
    h(
      "dl",
      { class: "facts" },
      fact("Formato", doc.paged ? `${format.toUpperCase()} · ${plural(doc.pages, "página", "páginas")}` : format.toUpperCase()),
      fact("Estado", stateLabel(doc)),
      fact("Revisión", !doc.converted ? "—" : doc.review_total === null ? "Por calcular" : doc.review_pending ? plural(doc.review_pending, "aviso pendiente", "avisos pendientes") : "Al día"),
      fact("Traducciones", ...translations),
    ),
    h(
      "div",
      { class: "inspector-actions" },
      primary,
      h(
        "div",
        { class: "quick" },
        quick("Leer", "page", "leer", true),
        quick("Traducir", "translate", "traducir", doc.converted || isEpub(doc)),
        quick("Exportar", "export", "exportar", doc.converted),
      ),
    ),
    button("Quitar de la biblioteca…", () => void removeSelected(), { class: "link remove-link" }),
  );
}

async function removeSelected(): Promise<void> {
  const doc = state.documents.find((d) => d.id === state.selectedId);
  if (!doc) return;
  const docs = visibleDocs();
  const index = docs.findIndex((d) => d.id === doc.id);
  if (await actions.remove(doc)) {
    const next = docs[index + 1] ?? docs[index - 1];
    state.selectedId = next?.id ?? null;
    renderLibrary();
  }
}

// --- Vista ------------------------------------------------------------------------------------

export function renderLibrary(): void {
  const docs = state.documents;
  const busy = docs.filter(FILTERS.busy).length;
  $("library-count").textContent = docs.length
    ? `${plural(docs.length, "documento", "documentos")}${busy ? ` · ${busy} en curso` : ""}`
    : "";
  for (const chip of document.querySelectorAll<HTMLButtonElement>("#library-filters [role=tab]")) {
    const name = chip.dataset.filter as Filter;
    chip.setAttribute("aria-selected", String(name === filter));
    const count = docs.filter(FILTERS[name]).length;
    (chip.querySelector("span") as HTMLSpanElement).textContent = String(count);
    chip.hidden = name !== "all" && count === 0 && name !== filter;
  }
  for (const head of document.querySelectorAll<HTMLButtonElement>(".table-head [data-sort]")) {
    if (head.dataset.sort === sort) head.setAttribute("aria-sort", ascending ? "ascending" : "descending");
    else head.removeAttribute("aria-sort");
  }
  const visible = visibleDocs();
  if (!state.selectedId || !docs.some((d) => d.id === state.selectedId)) state.selectedId = visible[0]?.id ?? null;
  $("library").replaceChildren(...visible.map(row));
  const empty = docs.length === 0;
  $("library-empty").hidden = !empty;
  $("library-body").hidden = empty;
  $("library-filters").hidden = empty;
  $("library-search-wrap").hidden = empty;
  $("library-add").hidden = empty;
  $("library-nomatch").hidden = empty || visible.length > 0;
  renderInspector();
}

export function focusSearch(): void {
  const input = $<HTMLInputElement>("library-search");
  input.focus();
  input.select();
}

export function libraryKey(event: KeyboardEvent): boolean {
  const docs = visibleDocs();
  if (!docs.length) return false;
  const index = docs.findIndex((d) => d.id === state.selectedId);
  const doc = docs[index];
  switch (event.key) {
    case "ArrowDown":
      select(docs[Math.min(index + 1, docs.length - 1)].id, true);
      return true;
    case "ArrowUp":
      select(docs[Math.max(index - 1, 0)].id, true);
      return true;
    case "Home":
      select(docs[0].id, true);
      return true;
    case "End":
      select(docs[docs.length - 1].id, true);
      return true;
    case "Enter":
      if (doc) void actions.open(doc);
      return true;
    case "Delete":
      void removeSelected();
      return true;
  }
  return false;
}

export function forgetThumb(id: string): void {
  const url = thumbs.get(id);
  if (url) URL.revokeObjectURL(url);
  thumbs.delete(id);
}

export function initLibrary(): void {
  for (const chip of document.querySelectorAll<HTMLButtonElement>("#library-filters [role=tab]")) {
    chip.addEventListener("click", () => {
      filter = chip.dataset.filter as Filter;
      renderLibrary();
    });
  }
  for (const head of document.querySelectorAll<HTMLButtonElement>(".table-head [data-sort]")) {
    head.addEventListener("click", () => {
      const value = head.dataset.sort as Sort;
      ascending = value === sort ? !ascending : value === "name";
      sort = value;
      try {
        localStorage.setItem("liblevo.sort", JSON.stringify({ sort, ascending }));
      } catch {
        // Recordar el orden es solo una comodidad.
      }
      renderLibrary();
    });
  }
  const search = $<HTMLInputElement>("library-search");
  search.addEventListener("input", () => {
    query = search.value.trim();
    renderLibrary();
  });
  search.addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      search.value = "";
      query = "";
      renderLibrary();
      search.blur();
    } else if (event.key === "ArrowDown") {
      event.preventDefault();
      const first = visibleDocs()[0];
      if (first) select(first.id, true);
    } else if (event.key === "Enter") {
      const first = visibleDocs()[0];
      if (first) void actions.open(first);
    }
  });
  $("clear-search").addEventListener("click", () => {
    search.value = "";
    query = "";
    filter = "all";
    renderLibrary();
  });
  $("library-add").addEventListener("click", () => void actions.addDocuments());
  $("empty-add").addEventListener("click", () => void actions.addDocuments());
  $("setup-open").addEventListener("click", () => actions.openComponents());
}
