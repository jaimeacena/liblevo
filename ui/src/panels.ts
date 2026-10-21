// Componentes, Actividad y Ajustes: lo que no depende de un documento.
import { api, type ComponentInfo } from "./api";
import { $, button, errorToast, h, history, ICON_OF, icon, onHistory } from "./dom";
import { actions, state, stem } from "./state";

// --- Componentes ------------------------------------------------------------------------------

const sizes = new Map<string, number>();
const names = new Map<string, string>();

export async function checkComponents(): Promise<ComponentInfo[]> {
  try {
    const components = await api.listComponents();
    const missing = components.some((c) => c.required && !c.installed);
    $("components-dot").hidden = !missing;
    $("setup-callout").hidden = !missing;
    state.installing = new Set(components.filter((c) => c.installing).map((c) => c.id));
    for (const c of components) {
      sizes.set(c.id, c.megabytes);
      names.set(c.id, c.name);
    }
    return components;
  } catch (error) {
    errorToast(error);
    return [];
  }
}

/** «312 de 1.152 MB · 27 %» para una descarga en curso. */
export function downloadText(id: string): string {
  const total = sizes.get(id) ?? 0;
  const share = state.componentProgress.get(id) ?? 0;
  return `${Math.round(total * share).toLocaleString("es-ES")} de ${total.toLocaleString("es-ES")} MB · ${Math.round(share * 100)} %`;
}

export async function renderComponents(): Promise<void> {
  const components = await checkComponents();
  $("component-list").replaceChildren(
    ...components.map((component) => {
      const installed = component.installed && !component.installing;
      const side = h("div", { class: "side" });
      if (component.installing) {
        const bar = h("progress", { max: 1, "data-id": component.id, "aria-label": `Descarga de ${component.name}` });
        bar.value = state.componentProgress.get(component.id) ?? 0;
        side.append(
          bar,
          h("span", { class: "subtle", "data-progress": component.id, text: downloadText(component.id) }),
          button("Cancelar", () => void api.cancelComponent(component.id), { class: "danger" }),
        );
      } else {
        side.append(
          button(installed ? "Volver a descargar" : "Descargar", async () => {
            try {
              await api.installComponent(component.id);
              state.installing.add(component.id);
              state.componentProgress.set(component.id, 0);
              await renderComponents();
              renderActivity();
            } catch (error) {
              errorToast(error);
            }
          }, { class: installed ? "ghost" : "primary" }),
        );
      }
      return h(
        "li",
        { class: installed ? "installed" : "" },
        h(
          "div",
          { class: "info" },
          h("strong", { text: component.name }),
          h("p", { text: component.description }),
          h(
            "div",
            { class: "meta-line" },
            installed
              ? h("span", { class: "pill ok" }, icon("check"), "Instalado y comprobado")
              : component.installing
                ? h("span", { class: "pill busy", text: "Descargando" })
                : h("span", { class: component.required ? "pill warn" : "pill muted", text: component.required ? "Necesario" : "Opcional" }),
            h("span", { class: "subtle", text: `${component.megabytes.toLocaleString("es-ES")} MB · ${component.license}` }),
          ),
        ),
        side,
      );
    }),
  );
}

export function openComponents(): void {
  const dialog = $<HTMLDialogElement>("components");
  void renderComponents();
  if (!dialog.open) dialog.showModal();
}

// --- Actividad --------------------------------------------------------------------------------

export function renderActivity(): void {
  const items: HTMLLIElement[] = [];
  for (const doc of state.documents) {
    if (doc.converting) {
      items.push(
        h(
          "li",
          {},
          h("div", { class: "row" }, h("strong", { title: doc.name, text: `Convirtiendo ${stem(doc)}` }), button("Cancelar", () => void api.cancelConversion(doc.id), { class: "ghost" })),
          h("progress", { max: 1 }),
          h("div", { class: "row spread" }, h("span", { class: "subtle", text: "Leyendo el documento…" }), button("Abrir", () => void actions.open(doc), { class: "link" })),
        ),
      );
    }
    if (doc.translating) {
      const [done, total] = state.translationProgress.get(doc.id) ?? [0, 0];
      const bar = h("progress", { max: 1 });
      if (total) bar.value = done / total;
      items.push(
        h(
          "li",
          {},
          h("div", { class: "row" }, h("strong", { title: doc.name, text: `Traduciendo ${stem(doc)}` }), button("Cancelar", () => void api.cancelTranslation(doc.id), { class: "ghost" })),
          bar,
          h("div", { class: "row spread" }, h("span", { class: "subtle", text: total ? `${done} de ${total} bloques` : "Preparando…" }), button("Abrir", () => void actions.openTab(doc, "traducir"), { class: "link" })),
        ),
      );
    }
  }
  for (const id of state.installing) {
    const bar = h("progress", { max: 1 });
    bar.value = state.componentProgress.get(id) ?? 0;
    items.push(
      h(
        "li",
        {},
        h("div", { class: "row" }, h("strong", { text: `Descargando ${names.get(id) ?? id}` }), button("Cancelar", () => void api.cancelComponent(id), { class: "ghost" })),
        bar,
        h("span", { class: "subtle", text: downloadText(id) }),
      ),
    );
  }
  $("activity-running").replaceChildren(...items);
  $("activity-idle").hidden = items.length > 0;
  const badge = $("activity-badge");
  badge.hidden = items.length === 0;
  badge.textContent = String(items.length);
  const time = (d: Date) => d.toLocaleTimeString("es-ES", { hour: "2-digit", minute: "2-digit" });
  $("activity-history").replaceChildren(
    ...(history.length
      ? history.slice(0, 20).map((entry) =>
          h("li", { class: entry.kind }, icon(ICON_OF[entry.kind]), h("span", { text: entry.message }), h("time", { text: time(entry.time) })),
        )
      : [h("li", { class: "subtle", text: "Aquí aparecerán los avisos de esta sesión." })]),
  );
}

export function toggleActivity(force?: boolean): void {
  const drawer = $("activity");
  const show = force ?? drawer.hidden;
  drawer.hidden = !show;
  $("nav-activity").setAttribute("aria-expanded", String(show));
  if (show) renderActivity();
}

// --- Ajustes ----------------------------------------------------------------------------------

type Theme = "system" | "light" | "dark";

function readTheme(): Theme {
  try {
    const value = localStorage.getItem("liblevo.theme");
    return value === "light" || value === "dark" ? value : "system";
  } catch {
    return "system";
  }
}

export function applyTheme(theme: Theme = readTheme()): void {
  if (theme === "system") document.documentElement.removeAttribute("data-theme");
  else document.documentElement.dataset.theme = theme;
  for (const option of document.querySelectorAll<HTMLButtonElement>("#theme [role=radio]")) {
    option.setAttribute("aria-checked", String(option.dataset.themeChoice === theme));
  }
}

export function initPanels(): void {
  for (const dialog of document.querySelectorAll<HTMLDialogElement>("dialog")) {
    dialog.querySelector("[data-close]")?.addEventListener("click", () => dialog.close());
    dialog.addEventListener("mousedown", (event) => {
      if (event.target === dialog) dialog.close();
    });
  }
  $("nav-components").addEventListener("click", openComponents);
  $("nav-activity").addEventListener("click", (event) => {
    event.stopPropagation();
    toggleActivity();
  });
  $("activity-close").addEventListener("click", () => toggleActivity(false));
  document.addEventListener("mousedown", (event) => {
    const drawer = $("activity");
    if (!drawer.hidden && !drawer.contains(event.target as Node) && !$("nav-activity").contains(event.target as Node)) {
      toggleActivity(false);
    }
  });
  $("nav-settings").addEventListener("click", () => $<HTMLDialogElement>("settings").showModal());
  $("settings-data").addEventListener("click", () => void api.revealData().catch(errorToast));
  for (const option of document.querySelectorAll<HTMLButtonElement>("#theme [role=radio]")) {
    option.addEventListener("click", () => {
      const theme = option.dataset.themeChoice as Theme;
      try {
        localStorage.setItem("liblevo.theme", theme);
      } catch {
        // Sin almacenamiento local, el tema vale solo para esta sesión.
      }
      applyTheme(theme);
    });
  }
  onHistory(() => {
    if (!$("activity").hidden) renderActivity();
  });
  applyTheme();
}
