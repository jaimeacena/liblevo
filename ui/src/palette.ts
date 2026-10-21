// Paleta de órdenes (Ctrl+K): abrir documentos y ejecutar acciones con el teclado.
import { $, h, icon } from "./dom";
import { actions, formatOf, isEpub, state, type Tab } from "./state";

interface Entry {
  group: "Documentos" | "Acciones" | "Ir a";
  label: string;
  icon: string;
  hint?: string;
  run: () => void;
}

let entries: Entry[] = [];
let active = 0;

const normal = (s: string) => s.normalize("NFD").replace(/\p{Diacritic}/gu, "").toLowerCase();

function build(query: string): Entry[] {
  const list: Entry[] = [];
  const doc = state.current;
  const number = Number(query.trim());
  if (doc?.paged && state.view === "doc" && Number.isInteger(number) && number >= 1 && number <= doc.pages) {
    list.push({ group: "Ir a", label: `Ir a la página ${number}`, icon: "page", run: () => void actions.goTo(number, null) });
  }
  if (doc && state.view === "doc") {
    const tabs: [Tab, string, string, boolean][] = [
      ["leer", "Leer", "page", true],
      ["revisar", "Revisar", "review", doc.converted],
      ["traducir", "Traducir", "translate", doc.converted || isEpub(doc)],
      ["exportar", "Exportar", "export", doc.converted],
      ["epub", "EPUB original", "edit", isEpub(doc)],
    ];
    tabs.forEach(([tab, label, iconName, enabled], index) => {
      if (enabled) list.push({ group: "Ir a", label, icon: iconName, hint: `Ctrl ${index + 1}`, run: () => actions.setTab(tab) });
    });
    if (!doc.converting) {
      list.push({
        group: "Acciones",
        label: doc.converted ? "Volver a convertir" : "Convertir",
        icon: "convert",
        run: () => void actions.convert(doc),
      });
    }
  }
  list.push(
    { group: "Acciones", label: "Añadir documentos", icon: "plus", hint: "Ctrl O", run: () => void actions.addDocuments() },
    { group: "Acciones", label: "Biblioteca", icon: "library", hint: "Alt ←", run: () => actions.showLibrary() },
    { group: "Acciones", label: "Componentes", icon: "box", run: () => actions.openComponents() },
    { group: "Acciones", label: "Ajustes", icon: "settings", run: () => $<HTMLDialogElement>("settings").showModal() },
  );
  for (const d of state.documents) {
    list.push({
      group: "Documentos",
      label: d.name,
      icon: "file",
      hint: formatOf(d.name).toUpperCase(),
      run: () => void actions.open(d),
    });
  }
  const q = normal(query.trim());
  if (!q || (Number.isInteger(number) && list[0]?.group === "Ir a")) return q ? list.filter((e) => e.group === "Ir a" || normal(e.label).includes(q)) : list;
  return list
    .filter((e) => normal(e.label).includes(q))
    .sort((a, b) => Number(!normal(a.label).startsWith(q)) - Number(!normal(b.label).startsWith(q)));
}

function render(): void {
  const list = $("palette-list");
  if (!entries.length) {
    list.replaceChildren(h("li", { class: "none", text: "Nada coincide." }));
    return;
  }
  const nodes: HTMLElement[] = [];
  let group = "";
  entries.slice(0, 60).forEach((entry, index) => {
    if (entry.group !== group) {
      group = entry.group;
      nodes.push(h("li", { class: "group", text: group }));
    }
    const item = h(
      "button",
      { type: "button", role: "option", "aria-selected": String(index === active), "data-index": index },
      icon(entry.icon),
      h("span", { text: entry.label }),
      entry.hint ? h("span", { class: "hint", text: entry.hint }) : null,
    );
    item.addEventListener("mousemove", () => {
      if (active !== index) {
        active = index;
        render();
      }
    });
    item.addEventListener("click", () => run(index));
    nodes.push(h("li", {}, item));
  });
  list.replaceChildren(...nodes);
  list.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: "nearest" });
}

function run(index: number): void {
  const entry = entries[index];
  if (!entry) return;
  $<HTMLDialogElement>("palette").close();
  entry.run();
}

export function openPalette(): void {
  const dialog = $<HTMLDialogElement>("palette");
  const input = $<HTMLInputElement>("palette-input");
  input.value = "";
  entries = build("");
  active = 0;
  render();
  if (!dialog.open) dialog.showModal();
  input.focus();
}

export function initPalette(): void {
  const input = $<HTMLInputElement>("palette-input");
  input.addEventListener("input", () => {
    entries = build(input.value);
    active = 0;
    render();
  });
  input.addEventListener("keydown", (event) => {
    if (event.key === "ArrowDown") {
      active = Math.min(active + 1, Math.min(entries.length, 60) - 1);
      render();
      event.preventDefault();
    } else if (event.key === "ArrowUp") {
      active = Math.max(active - 1, 0);
      render();
      event.preventDefault();
    } else if (event.key === "Enter") {
      run(active);
      event.preventDefault();
    }
  });
  $("nav-palette").addEventListener("click", openPalette);
}
