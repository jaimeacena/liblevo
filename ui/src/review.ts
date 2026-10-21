// Revisar: cola de avisos ordenada por riesgo y detalle de cada uno junto al original.
import { api, type ReviewEntry, type ReviewView } from "./api";
import { $, button, errorToast, frag, h, toast } from "./dom";
import { actions, plural, state } from "./state";

type Filter = "pending" | "all" | "done";

let view: ReviewView | null = null;
let filter: Filter = "pending";
let selected: string | null = null;
let editing = false;
let pageUrl: string | null = null;
let loadedFor: string | null = null;
/** HTML del resultado por página, para mostrar el fragmento ya formateado. */
let pages: Map<number, string> | null = null;
let detailToken = 0;

const DECISIONS: Record<string, string> = {
  correcto: "Correcto",
  corregido: "Corregido",
  despues: "Para después",
};

const isPending = (i: ReviewEntry) => !i.decision || i.decision === "despues";

function visible(): ReviewEntry[] {
  if (!view) return [];
  if (filter === "pending") return view.items.filter(isPending);
  if (filter === "done") return view.items.filter((i) => !isPending(i));
  return view.items;
}

export async function loadReview(force = false): Promise<void> {
  const doc = state.current;
  if (!doc?.converted) return;
  if (!force && loadedFor === doc.id && view) {
    renderReview();
    return;
  }
  try {
    view = await api.reviewList(doc.id);
    loadedFor = doc.id;
    pages = null;
    if (!selected || !view.items.some((i) => i.key === selected)) selected = visible()[0]?.key ?? null;
    renderReview();
  } catch (error) {
    errorToast(error);
  }
}

export function resetReview(): void {
  view = null;
  pages = null;
  loadedFor = null;
  selected = null;
  editing = false;
  filter = "pending";
}

function renderSummary(): void {
  if (!view) return;
  const total = view.items.length;
  const pending = view.items.filter(isPending).length;
  const decided = total - pending;
  const share = view.total_risk ? Math.round((100 * view.pending_risk) / view.total_risk) : 0;
  $("review-summary").textContent = `${decided.toLocaleString("es-ES")} de ${plural(total, "aviso revisado", "avisos revisados")}`;
  $("review-risk").textContent = pending ? `Riesgo pendiente ${share} %` : "";
  $("review-meter").style.width = `${total ? (100 * decided) / total : 0}%`;
  for (const option of document.querySelectorAll<HTMLButtonElement>("#review-filter [role=radio]")) {
    option.setAttribute("aria-checked", String(option.dataset.filter === filter));
  }
}

function renderQueue(): void {
  const items = visible();
  const max = Math.max(1, ...items.map((i) => i.risk));
  $("review-list").replaceChildren(
    ...items.map((item) => {
      const cls = !item.decision ? "" : item.decision === "despues" ? "later" : "decided";
      const node = button(
        frag(
          h("span", { class: "q-dot" }),
          h("span", {
            class: "q-title",
            text: `Página ${item.page}${item.block === null ? " · entera" : ""}${item.decision ? ` · ${DECISIONS[item.decision] ?? item.decision}` : ""}`,
          }),
          h("span", { class: "q-risk", title: "Riesgo relativo", text: "●".repeat(Math.max(1, Math.round((3 * item.risk) / max))) }),
          h("span", { class: "q-text", text: item.reasons[0] ? `${item.reasons[0]} — ${item.excerpt}` : item.excerpt }),
        ),
        () => select(item.key),
        { "aria-current": String(item.key === selected), "data-key": item.key },
      );
      return h("li", { class: cls }, node);
    }),
  );
  if (!items.length) {
    $("review-list").replaceChildren(
      h("li", { class: "mini-empty" }, filter === "done" ? "Todavía no has decidido ningún aviso." : "No hay avisos pendientes."),
    );
  }
  $("review-list").querySelector('[aria-current="true"]')?.scrollIntoView({ block: "nearest" });
}

function renderDone(): boolean {
  const total = view?.items.length ?? 0;
  const pending = view?.items.filter(isPending).length ?? 0;
  const done = total === 0 || (pending === 0 && filter === "pending");
  $("review-done").hidden = !done;
  $("review-queue").hidden = done;
  $("review-detail").hidden = done;
  document.querySelector<HTMLElement>('[data-panel="revisar"]')?.classList.toggle("complete", done);
  if (!done) return false;
  $("review-done-title").textContent = total === 0 ? "Sin avisos" : "Todo revisado";
  $("review-done-text").textContent =
    total === 0
      ? "La conversión no ha encontrado nada dudoso. Que no haya avisos no garantiza que todo esté bien: conviene leer el resultado."
      : `Has decidido los ${total.toLocaleString("es-ES")} avisos. El documento está listo para exportar.`;
  $("review-done-all").hidden = total === 0;
  return true;
}

async function renderDetail(): Promise<void> {
  const items = visible();
  const item = view?.items.find((i) => i.key === selected) ?? null;
  const doc = state.current;
  $("review-item").hidden = !item;
  if (!item || !doc) return;
  const token = ++detailToken;
  const index = items.findIndex((i) => i.key === item.key);
  $("review-position").textContent = index >= 0 ? `Aviso ${index + 1} de ${items.length}` : "Fuera del filtro";
  $("review-title").textContent =
    (item.block === null ? `Página ${item.page} entera` : `Página ${item.page}`) +
    (item.decision ? ` · ${DECISIONS[item.decision] ?? item.decision}` : "");
  $("review-page-number").textContent = String(item.page);
  $("review-reasons").replaceChildren(...item.reasons.map((r) => h("li", { text: r })));
  $<HTMLButtonElement>("review-prev").disabled = index <= 0;
  $<HTMLButtonElement>("review-next").disabled = index < 0 || index >= items.length - 1;
  setEditing(editing, item);

  // El fragmento tal como se ve en el resultado; el extracto si es una página entera.
  const text = $("review-text");
  text.classList.remove("result");
  text.textContent = item.excerpt;
  if (item.block !== null) {
    try {
      if (!pages) {
        pages = new Map();
        for (const v of await api.resultPages(doc.id, null)) pages.set(v.page, (pages.get(v.page) ?? "") + v.html);
      }
      if (token !== detailToken) return;
      const scratch = document.createElement("div");
      // HTML generado por el núcleo: el HTML incrustado del documento ya es texto.
      scratch.innerHTML = pages.get(item.page) ?? "";
      const block = scratch.querySelector(`.block[data-block="${item.block}"]`);
      if (block) {
        text.innerHTML = block.innerHTML;
        text.classList.add("result");
      }
    } catch {
      text.textContent = item.excerpt;
    }
  }
  // Original de la página, si es un PDF.
  const page = $("review-page");
  const figure = page.closest("figure") as HTMLElement;
  figure.hidden = !doc.paged || !doc.available;
  if (doc.paged && doc.available) {
    page.classList.add("loading");
    try {
      const width = Math.round(Math.min(page.clientWidth || 600, 1400) * window.devicePixelRatio);
      const bytes = await api.renderPage(doc.id, item.page, width);
      if (token !== detailToken) return;
      if (pageUrl) URL.revokeObjectURL(pageUrl);
      pageUrl = URL.createObjectURL(new Blob([bytes], { type: "image/png" }));
      page.replaceChildren(h("img", { src: pageUrl, alt: `Página ${item.page} del original` }));
    } catch (error) {
      page.replaceChildren(h("p", { class: "subtle", text: String(error) }));
    } finally {
      if (token === detailToken) page.classList.remove("loading");
    }
  }
}

function setEditing(on: boolean, item: ReviewEntry): void {
  editing = on;
  $("review-text").hidden = on;
  $("review-edit").hidden = !on;
  $("review-save").hidden = !on;
  $("review-cancel-edit").hidden = !on;
  $("review-ok").hidden = on;
  $("review-later").hidden = on;
  $("review-fix").hidden = on || item.block === null;
}

export function renderReview(): void {
  renderSummary();
  if (renderDone()) return;
  renderQueue();
  void renderDetail();
}

function select(key: string | null): void {
  selected = key;
  editing = false;
  renderQueue();
  void renderDetail();
}

function move(delta: number): void {
  const items = visible();
  const index = items.findIndex((i) => i.key === selected);
  const next = items[Math.min(Math.max(index + delta, 0), items.length - 1)];
  if (next) select(next.key);
}

async function decide(decision: string, text: string | null = null): Promise<void> {
  const doc = state.current;
  const item = view?.items.find((i) => i.key === selected);
  if (!doc || !item) return;
  const before = visible();
  const index = before.findIndex((i) => i.key === item.key);
  try {
    view = await api.reviewDecide(doc.id, item, decision, text);
    const after = visible();
    const stays = after.some((i) => i.key === item.key);
    const next = stays
      ? before[index + 1]
      : (before.slice(index + 1).find((i) => after.some((a) => a.key === i.key)) ?? after[Math.max(0, index - 1)]);
    selected = next?.key ?? (stays ? item.key : null);
    editing = false;
    if (decision === "corregido") {
      pages = null;
      toast(`Corrección guardada en la página ${item.page}.`, "success");
    }
    renderReview();
    void actions.refresh(doc.id);
  } catch (error) {
    errorToast(error);
  }
}

/** Abre un aviso concreto (desde un fragmento marcado en Leer). */
export async function focusItem(page: number, block: number | null): Promise<void> {
  await loadReview();
  const item = view?.items.find((i) => i.page === page && i.block === block);
  if (!item) return;
  if (!visible().some((i) => i.key === item.key)) filter = "all";
  selected = item.key;
  renderReview();
}

export function reviewKey(event: KeyboardEvent): boolean {
  if (!view) return false;
  if (editing) {
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "s") {
      void decide("corregido", $<HTMLTextAreaElement>("review-edit").value);
      return true;
    }
    if (event.key === "Escape") {
      const item = view.items.find((i) => i.key === selected);
      if (item) setEditing(false, item);
      return true;
    }
    return false;
  }
  if (event.ctrlKey || event.metaKey || event.altKey) return false;
  switch (event.key.toLowerCase()) {
    case "j":
    case "arrowdown":
      move(1);
      return true;
    case "k":
    case "arrowup":
      move(-1);
      return true;
    case "c":
      void decide("correcto");
      return true;
    case "d":
      void decide("despues");
      return true;
    case "e":
      void startEdit();
      return true;
    case "l": {
      const item = view.items.find((i) => i.key === selected);
      if (item) void actions.goTo(item.page, item.block);
      return true;
    }
  }
  return false;
}

/** El campo de corrección acepta Ctrl+S y Esc aunque tenga el foco. */
export function reviewEditing(): boolean {
  return editing;
}

async function startEdit(): Promise<void> {
  const item = view?.items.find((i) => i.key === selected);
  if (!item || item.block === null || !state.current) return;
  try {
    $<HTMLTextAreaElement>("review-edit").value = await api.reviewBlockText(state.current.id, item.page, item.block);
  } catch (error) {
    errorToast(error);
    return;
  }
  setEditing(true, item);
  $("review-edit").focus();
}

export function initReview(): void {
  for (const option of document.querySelectorAll<HTMLButtonElement>("#review-filter [role=radio]")) {
    option.addEventListener("click", () => {
      filter = option.dataset.filter as Filter;
      if (!visible().some((i) => i.key === selected)) selected = visible()[0]?.key ?? null;
      renderReview();
    });
  }
  $("review-prev").addEventListener("click", () => move(-1));
  $("review-next").addEventListener("click", () => move(1));
  $("review-ok").addEventListener("click", () => void decide("correcto"));
  $("review-later").addEventListener("click", () => void decide("despues"));
  $("review-fix").addEventListener("click", () => void startEdit());
  $("review-cancel-edit").addEventListener("click", () => {
    const item = view?.items.find((i) => i.key === selected);
    if (item) setEditing(false, item);
  });
  $("review-save").addEventListener("click", () => void decide("corregido", $<HTMLTextAreaElement>("review-edit").value));
  $("review-open").addEventListener("click", () => {
    const item = view?.items.find((i) => i.key === selected);
    if (item) void actions.goTo(item.page, item.block);
  });
  $("review-done-read").addEventListener("click", () => actions.setTab("leer"));
  $("review-done-export").addEventListener("click", () => actions.setTab("exportar"));
  $("review-done-all").addEventListener("click", () => {
    filter = "all";
    selected = visible()[0]?.key ?? null;
    renderReview();
  });
}
