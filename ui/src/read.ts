// Leer: el original junto a su texto (o el texto junto a su traducción), página a página.
import { api } from "./api";
import { $, button, errorToast, frag, h, toast } from "./dom";
import { actions, plural, state } from "./state";

type Layout = "original" | "text" | "compare";

let layout: Layout = "original";
/** 1 = ancho del panel; `fitPage` ajusta la página entera a la altura disponible. */
let zoom = 1;
let fitPage = false;
let outlineOpen = false;
/** Texto del panel principal (resultado o traducción) y del de comparación, por página. */
let mainPages = new Map<number, string>();
let comparePages = new Map<number, string>();
let compareLanguage: string | null = null;
let highlight: number | null = null;
let objectUrl: string | null = null;
/** Fragmentos con avisos pendientes: «página:bloque» → motivos, y páginas con avisos. */
let flagged = new Map<string, string[]>();
let flaggedPages = new Set<number>();
let loadedFor: string | null = null;
let loading = false;
/** Evita que una página lenta pise a la siguiente. */
let drawToken = 0;
let editing = false;
let previewTimer = 0;

const ZOOMS = [0.5, 0.75, 1, 1.25, 1.5, 2, 2.5];

try {
  outlineOpen = localStorage.getItem("liblevo.outline") === "1";
  fitPage = localStorage.getItem("liblevo.fitPage") === "1";
} catch {
  // Preferencias de la sesión.
}

function remember(key: string, value: boolean): void {
  try {
    localStorage.setItem(key, value ? "1" : "0");
  } catch {
    // Recordarlo es solo una comodidad.
  }
}

async function pagesOf(id: string, translation: string | null): Promise<Map<number, string>> {
  const map = new Map<number, string>();
  for (const view of await api.resultPages(id, translation)) {
    map.set(view.page, (map.get(view.page) ?? "") + view.html);
  }
  return map;
}

function skeleton(target: HTMLElement): void {
  target.replaceChildren(
    h("div", { class: "skeleton-lines" }, ...Array.from({ length: 9 }, () => h("span", { class: "skeleton" }))),
  );
}

/** Recarga los textos del documento actual (tras convertir, corregir o traducir). */
export async function reloadText(): Promise<void> {
  const doc = state.current;
  if (!doc?.converted) {
    mainPages = new Map();
    comparePages = new Map();
    return;
  }
  loading = true;
  skeleton($("result"));
  try {
    if (layout === "compare") {
      compareLanguage = compareLanguage ?? state.viewing ?? doc.translations[0]?.language ?? null;
      [mainPages, comparePages] = await Promise.all([
        pagesOf(doc.id, null),
        compareLanguage ? pagesOf(doc.id, compareLanguage) : Promise.resolve(new Map<number, string>()),
      ]);
    } else {
      mainPages = await pagesOf(doc.id, state.viewing);
    }
    const review = await api.reviewList(doc.id);
    const pending = review.items.filter((i) => !i.decision || i.decision === "despues");
    flagged = new Map(pending.filter((i) => i.block !== null).map((i) => [`${i.page}:${i.block}`, i.reasons]));
    flaggedPages = new Set(pending.map((i) => i.page));
  } catch (error) {
    errorToast(error);
  } finally {
    loading = false;
  }
  renderOutline();
  renderScrubber();
}

function fill(target: HTMLElement, html: string | undefined, markFlags: boolean): void {
  // El HTML viene del núcleo: el HTML incrustado del documento ya se ha convertido en texto.
  target.innerHTML = html ?? '<p class="missing-page">Esta página no tiene texto en el resultado.</p>';
  if (!markFlags) return;
  for (const block of target.querySelectorAll<HTMLElement>(".block[data-block]")) {
    const reasons = flagged.get(`${state.page}:${block.dataset.block}`);
    if (!reasons) continue;
    block.classList.add("flagged");
    block.title = `Aviso: ${reasons.join(" · ")}. Haz clic para revisarlo.`;
    block.addEventListener("click", () => actions.reviewAt(state.page, Number(block.dataset.block)));
  }
}

function labelFor(language: string | null): string {
  if (!language) return "Resultado";
  const t = state.current?.translations.find((x) => x.language === language);
  return `Traducción · ${t?.label ?? language}`;
}

/** Dibuja la página actual en los paneles según la disposición. */
export async function renderPage(): Promise<void> {
  const doc = state.current;
  if (!doc) return;
  if (loadedFor !== doc.id) {
    loadedFor = doc.id;
    await reloadText();
  }
  const paged = doc.paged;
  const effective: Layout = !paged && layout === "original" ? "text" : layout;
  const body = $("read-body");
  body.dataset.layout = effective;
  body.classList.toggle("with-outline", outlineOpen && doc.converted);
  body.classList.toggle("editing", editing);
  $("outline").hidden = !outlineOpen || !doc.converted;
  $("toggle-outline").setAttribute("aria-pressed", String(outlineOpen));
  $<HTMLButtonElement>("toggle-outline").disabled = !doc.converted;
  $("pager").hidden = !paged;
  $("zoom").hidden = effective !== "original";
  const pageInput = $<HTMLInputElement>("page-input");
  pageInput.value = String(state.page);
  pageInput.max = String(doc.pages);
  $("page-total").textContent = `de ${doc.pages}`;
  $<HTMLButtonElement>("prev").disabled = state.page <= 1;
  $<HTMLButtonElement>("next").disabled = state.page >= doc.pages;
  renderScrubber();

  const notConverted = !doc.converted;
  $("not-converted").hidden = !notConverted;
  $("result").hidden = notConverted;
  $<HTMLButtonElement>("edit-page").hidden = notConverted || state.viewing !== null || effective === "compare";

  const resultA = $("result-a");
  const pageView = $("page");
  if (effective === "compare") {
    $("pane-a-title").textContent = "Resultado";
    $("pane-b-title").textContent = labelFor(compareLanguage);
    pageView.hidden = true;
    resultA.hidden = false;
    fill(resultA, mainPages.get(state.page), true);
    fill($("result"), comparePages.get(state.page), false);
  } else {
    $("pane-a-title").textContent = "Original";
    $("pane-b-title").textContent = labelFor(state.viewing);
    pageView.hidden = false;
    resultA.hidden = true;
    if (!notConverted && !loading) fill($("result"), mainPages.get(state.page), state.viewing === null);
  }
  // Cada página empieza arriba, salvo que se vaya a un fragmento concreto.
  if (highlight === null) {
    $("pane-a").scrollTop = 0;
    $("pane-b").scrollTop = 0;
  } else {
    const target = document.querySelector(`#result .block[data-block="${highlight}"], #result-a .block[data-block="${highlight}"]`);
    target?.classList.add("highlight");
    target?.scrollIntoView({ block: "center", behavior: "smooth" });
    highlight = null;
  }
  markOutline();
  if (paged && effective === "original") await drawOriginal();
}

function zoomLabel(): string {
  if (fitPage) return "Página";
  return zoom === 1 ? "Ancho" : `${Math.round(zoom * 100)} %`;
}

async function drawOriginal(): Promise<void> {
  const doc = state.current;
  if (!doc) return;
  const token = ++drawToken;
  const pageView = $("page");
  pageView.style.setProperty("--zoom", String(zoom));
  pageView.classList.toggle("fit", fitPage);
  $("zoom-value").textContent = zoomLabel();
  $("zoom-fit").setAttribute("aria-pressed", String(fitPage));
  const pane = $("pane-a");
  const base = fitPage ? Math.min(pane.clientWidth, (window.innerHeight - 290) * 0.75) : pane.clientWidth * zoom;
  const width = Math.round(Math.min(base || 700, 2400) * window.devicePixelRatio);
  if (!pageView.querySelector("img")) pageView.classList.add("loading");
  try {
    const bytes = await api.renderPage(doc.id, state.page, Math.min(width, 3000));
    if (token !== drawToken) return;
    if (objectUrl) URL.revokeObjectURL(objectUrl);
    objectUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
    pageView.replaceChildren(h("img", { src: objectUrl, alt: `Página ${state.page} del original` }));
  } catch (error) {
    if (token === drawToken) pageView.replaceChildren(h("div", { class: "empty-card" }, h("p", { text: String(error) })));
  } finally {
    if (token === drawToken) pageView.classList.remove("loading");
  }
}

export async function showPage(page: number, block: number | null = null): Promise<void> {
  const doc = state.current;
  if (!doc) return;
  const target = Math.min(Math.max(page, 1), Math.max(doc.pages, 1));
  if (target === state.page && block === null && !editing && loadedFor === doc.id) return;
  state.page = target;
  highlight = block;
  if (editing) closeEditor();
  await renderPage();
}

// --- Barra de páginas -------------------------------------------------------------------------

function renderScrubber(): void {
  const doc = state.current;
  const bar = $("scrubber");
  if (!doc || !doc.paged || doc.pages < 2) {
    bar.hidden = true;
    return;
  }
  bar.hidden = false;
  const position = (page: number) => `${((page - 1) / (doc.pages - 1)) * 100}%`;
  $("scrubber-thumb").style.left = position(state.page);
  const ticks = $("scrubber-ticks");
  if (ticks.dataset.for !== `${doc.id}:${flaggedPages.size}`) {
    ticks.dataset.for = `${doc.id}:${flaggedPages.size}`;
    ticks.replaceChildren(
      ...[...flaggedPages].map((page) => {
        const tick = document.createElement("i");
        tick.style.left = position(page);
        return tick;
      }),
    );
  }
  $("scrubber-legend").replaceChildren(
    ...(flaggedPages.size ? [document.createElement("i"), plural(flaggedPages.size, "página con avisos", "páginas con avisos")] : []),
  );
}

function pageAt(event: MouseEvent): number {
  const doc = state.current;
  const track = $("scrubber-track").getBoundingClientRect();
  if (!doc) return 1;
  const ratio = Math.min(Math.max((event.clientX - track.left) / track.width, 0), 1);
  return Math.round(ratio * (doc.pages - 1)) + 1;
}

// --- Índice -----------------------------------------------------------------------------------

interface Heading {
  level: number;
  text: string;
  page: number;
}

function renderOutline(): void {
  const headings: Heading[] = [];
  const scratch = document.createElement("div");
  for (const [page, html] of [...mainPages.entries()].sort((a, b) => a[0] - b[0])) {
    scratch.innerHTML = html;
    for (const node of scratch.querySelectorAll("h1, h2, h3")) {
      const text = node.textContent?.trim();
      if (text) headings.push({ level: Number(node.tagName[1]), text, page });
    }
    if (headings.length >= 800) break;
  }
  const list = $("outline-list");
  if (!headings.length) {
    list.replaceChildren(h("li", { class: "empty", text: "Este texto no tiene títulos." }));
    return;
  }
  const top = Math.min(...headings.map((x) => x.level));
  list.replaceChildren(
    ...headings.map((x) =>
      h(
        "li",
        {},
        button(frag(h("span", { text: x.text }), h("span", { class: "pg", text: String(x.page) })), () => void showPage(x.page), {
          class: `level-${Math.min(x.level - top + 1, 3)}`,
          "data-page": x.page,
        }),
      ),
    ),
  );
  markOutline();
}

function markOutline(): void {
  if (!outlineOpen) return;
  let current: HTMLElement | null = null;
  for (const item of $("outline-list").querySelectorAll<HTMLButtonElement>("button[data-page]")) {
    item.classList.remove("current");
    if (Number(item.dataset.page) <= state.page) current = item;
  }
  current?.classList.add("current");
  current?.scrollIntoView({ block: "nearest" });
}

export function toggleOutline(): void {
  if (!state.current?.converted) return;
  outlineOpen = !outlineOpen;
  remember("liblevo.outline", outlineOpen);
  void renderPage();
}

// --- Selector de texto y disposición ----------------------------------------------------------

export function syncControls(): void {
  const doc = state.current;
  if (!doc) return;
  const select = $<HTMLSelectElement>("view-select");
  const hasTranslations = doc.translations.length > 0;
  if (state.viewing && !doc.translations.some((t) => t.language === state.viewing)) state.viewing = null;
  if (layout === "compare" && !hasTranslations) layout = doc.paged ? "original" : "text";
  select.replaceChildren(
    ...(layout === "compare" ? [] : [new Option("Resultado", "")]),
    ...doc.translations.map((t) => new Option(`Traducción · ${t.label}`, t.language)),
  );
  select.value = layout === "compare" ? (compareLanguage ?? doc.translations[0]?.language ?? "") : (state.viewing ?? "");
  select.hidden = !hasTranslations || !doc.converted;
  const effective = !doc.paged && layout === "original" ? "text" : layout;
  for (const option of document.querySelectorAll<HTMLButtonElement>("#layout [role=radio]")) {
    const value = option.dataset.layout as Layout;
    option.hidden = (value === "original" && !doc.paged) || (value === "compare" && !hasTranslations);
    option.setAttribute("aria-checked", String(value === effective));
  }
  const visible = [...document.querySelectorAll<HTMLButtonElement>("#layout [role=radio]")].filter((o) => !o.hidden);
  $("layout").hidden = visible.length < 2 || !doc.converted;
}

/** Cambia de documento: vuelve a los valores iniciales (salvo preferencias recordadas). */
export function resetRead(): void {
  loadedFor = null;
  compareLanguage = null;
  layout = "original";
  zoom = 1;
  flagged = new Map();
  flaggedPages = new Set();
  $("scrubber-ticks").dataset.for = "";
  if (editing) closeEditor();
}

export async function setLayout(value: Layout): Promise<void> {
  layout = value;
  if (value !== "compare") compareLanguage = null;
  syncControls();
  await reloadText();
  await renderPage();
}

function setZoom(delta: number | "fit"): void {
  if (delta === "fit") {
    fitPage = !fitPage;
    remember("liblevo.fitPage", fitPage);
  } else {
    fitPage = false;
    const index = Math.min(Math.max(ZOOMS.indexOf(zoom) + delta, 0), ZOOMS.length - 1);
    zoom = ZOOMS[index];
  }
  void drawOriginal();
}

// --- Editor de página -------------------------------------------------------------------------

export function isEditing(): boolean {
  return editing;
}

function closeEditor(): void {
  editing = false;
  $("editor").hidden = true;
  $("read-body").classList.remove("editing");
}

async function updatePreview(): Promise<void> {
  const doc = state.current;
  if (!doc) return;
  try {
    // Vista previa generada por el núcleo, con el mismo saneado que el resultado.
    $("editor-preview").innerHTML = await api.previewMarkdown(doc.id, $<HTMLTextAreaElement>("editor-text").value);
  } catch {
    // Si falla la vista previa, el editor sigue funcionando.
  }
}

async function openEditor(): Promise<void> {
  const doc = state.current;
  if (!doc) return;
  try {
    $<HTMLTextAreaElement>("editor-text").value = await api.pageMarkdown(doc.id, state.page);
    $("editor-title").textContent = doc.paged ? `Editar la página ${state.page}` : "Editar el texto";
    editing = true;
    $("editor").hidden = false;
    $("read-body").classList.add("editing");
    await updatePreview();
    $("editor-text").focus();
  } catch (error) {
    errorToast(error);
  }
}

async function saveEditor(): Promise<void> {
  const doc = state.current;
  if (!doc || !editing) return;
  try {
    await api.savePage(doc.id, state.page, $<HTMLTextAreaElement>("editor-text").value);
    closeEditor();
    await reloadText();
    await renderPage();
    toast(doc.paged ? `Página ${state.page} guardada en el resultado.` : "Texto guardado en el resultado.", "success");
  } catch (error) {
    errorToast(error);
  }
}

export function editorKey(event: KeyboardEvent): boolean {
  if (!editing) return false;
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
    void saveEditor();
    return true;
  }
  if (event.key === "Escape") {
    closeEditor();
    void renderPage();
    return true;
  }
  return false;
}

export function readKey(event: KeyboardEvent): boolean {
  const doc = state.current;
  if (!doc || editing) return false;
  switch (event.key) {
    case "ArrowLeft":
    case "PageUp":
      void showPage(state.page - 1);
      return true;
    case "ArrowRight":
    case "PageDown":
      void showPage(state.page + 1);
      return true;
    case "Home":
      void showPage(1);
      return true;
    case "End":
      void showPage(doc.pages);
      return true;
    case "+":
    case "=":
      setZoom(1);
      return true;
    case "-":
      setZoom(-1);
      return true;
    case "0":
      setZoom("fit");
      return true;
  }
  if (event.key.toLowerCase() === "i") {
    toggleOutline();
    return true;
  }
  return false;
}

export function initRead(): void {
  $("prev").addEventListener("click", () => void showPage(state.page - 1));
  $("next").addEventListener("click", () => void showPage(state.page + 1));
  const pageInput = $<HTMLInputElement>("page-input");
  pageInput.addEventListener("change", () => {
    const value = Number(pageInput.value);
    if (Number.isInteger(value)) void showPage(value);
  });
  pageInput.addEventListener("focus", () => pageInput.select());
  $("toggle-outline").addEventListener("click", toggleOutline);
  for (const option of document.querySelectorAll<HTMLButtonElement>("#layout [role=radio]")) {
    option.addEventListener("click", () => void setLayout(option.dataset.layout as Layout));
  }
  $("view-select").addEventListener("change", async (event) => {
    const value = (event.target as HTMLSelectElement).value || null;
    if (layout === "compare") compareLanguage = value;
    else state.viewing = value;
    syncControls();
    await reloadText();
    await renderPage();
  });
  $("zoom-in").addEventListener("click", () => setZoom(1));
  $("zoom-out").addEventListener("click", () => setZoom(-1));
  $("zoom-fit").addEventListener("click", () => setZoom("fit"));
  $("edit-page").addEventListener("click", () => void openEditor());
  $("editor-cancel").addEventListener("click", () => {
    closeEditor();
    void renderPage();
  });
  $("editor-save").addEventListener("click", () => void saveEditor());
  $("editor-text").addEventListener("input", () => {
    window.clearTimeout(previewTimer);
    previewTimer = window.setTimeout(() => void updatePreview(), 250);
  });
  $("convert-inline").addEventListener("click", () => {
    if (state.current) void actions.convert(state.current);
  });
  const track = $("scrubber-track");
  const tip = $("scrubber-tip");
  track.addEventListener("mousemove", (event) => {
    const page = pageAt(event);
    tip.hidden = false;
    tip.textContent = `Página ${page}${flaggedPages.has(page) ? " · con avisos" : ""}`;
    tip.style.left = `${event.clientX - track.getBoundingClientRect().left}px`;
  });
  track.addEventListener("mouseleave", () => (tip.hidden = true));
  track.addEventListener("click", (event) => void showPage(pageAt(event)));
  window.addEventListener("resize", () => {
    if (state.view === "doc" && state.tab === "leer" && !editing) void drawOriginal();
  });
}
