// Ayudas de DOM: creación de elementos, iconos, confirmaciones y notificaciones.

export const $ = <T extends HTMLElement = HTMLElement>(id: string) =>
  document.getElementById(id) as T;

type Child = Node | string | null | undefined | false;
type Props = Record<string, string | number | boolean | undefined>;

/** Crea un elemento con atributos y descendientes. `class` y `text` son atajos. */
export function h<K extends keyof HTMLElementTagNameMap>(
  tag: K,
  props: Props = {},
  ...children: Child[]
): HTMLElementTagNameMap[K] {
  const node = document.createElement(tag);
  for (const [key, value] of Object.entries(props)) {
    if (value === undefined || value === false) continue;
    if (key === "class") node.className = String(value);
    else if (key === "text") node.textContent = String(value);
    else node.setAttribute(key, value === true ? "" : String(value));
  }
  for (const child of children) {
    if (child === null || child === undefined || child === false) continue;
    node.append(child);
  }
  return node;
}

/** Agrupa nodos y textos sin crear un elemento. */
export function frag(...children: Child[]): DocumentFragment {
  const fragment = document.createDocumentFragment();
  for (const child of children) {
    if (child === null || child === undefined || child === false) continue;
    fragment.append(child);
  }
  return fragment;
}

export function button(label: Child, onClick: (event: MouseEvent) => void, props: Props = {}): HTMLButtonElement {
  const node = h("button", { type: "button", ...props }, label);
  node.addEventListener("click", onClick);
  return node;
}

const ICONS: Record<string, string> = {
  library: "M4 3h3v18H4V3Zm5 0h3v18H9V3Zm5.2 1.3 2.9-.8 4.7 17.4-2.9.8-4.7-17.4Z",
  activity: "M3 12h4l3-8 4 16 3-8h4v2h-2.6L14 24 10 8l-1.6 6H3v-2Z",
  box: "M12 2 3 7v10l9 5 9-5V7l-9-5Zm0 2.3L18.7 8 12 11.7 5.3 8 12 4.3ZM5 9.7l6 3.3v6.7l-6-3.3V9.7Zm8 10V13l6-3.3v6.7l-6 3.3Z",
  settings:
    "M19.4 13a7.6 7.6 0 0 0 0-2l2.1-1.6-2-3.5-2.5 1a7.4 7.4 0 0 0-1.7-1L14.9 3h-4l-.4 2.9a7.4 7.4 0 0 0-1.7 1l-2.5-1-2 3.5L6.4 11a7.6 7.6 0 0 0 0 2l-2.1 1.6 2 3.5 2.5-1a7.4 7.4 0 0 0 1.7 1l.4 2.9h4l.4-2.9a7.4 7.4 0 0 0 1.7-1l2.5 1 2-3.5L19.4 13ZM12.9 15.5a3.5 3.5 0 1 1 0-7 3.5 3.5 0 0 1 0 7Z",
  command:
    "M6.5 3A3.5 3.5 0 0 0 3 6.5 3.5 3.5 0 0 0 6.5 10H8v4H6.5A3.5 3.5 0 1 0 10 17.5V16h4v1.5a3.5 3.5 0 1 0 3.5-3.5H16v-4h1.5a3.5 3.5 0 1 0-3.5-3.5V8h-4V6.5A3.5 3.5 0 0 0 6.5 3Zm0 2A1.5 1.5 0 0 1 8 6.5V8H6.5a1.5 1.5 0 0 1 0-3Zm11 0a1.5 1.5 0 1 1 0 3H16V6.5A1.5 1.5 0 0 1 17.5 5ZM10 10h4v4h-4v-4Zm-3.5 6H8v1.5A1.5 1.5 0 1 1 6.5 16Zm9.5 0h1.5a1.5 1.5 0 1 1-1.5 1.5V16Z",
  plus: "M11 5h2v6h6v2h-6v6h-2v-6H5v-2h6V5Z",
  minus: "M5 11h14v2H5v-2Z",
  back: "m11 5-7 7 7 7 1.4-1.4L7.8 13H20v-2H7.8l4.6-4.6L11 5Z",
  left: "m15 5-7 7 7 7 1.4-1.4L10.8 12l5.6-5.6L15 5Z",
  right: "m9 5 7 7-7 7-1.4-1.4 5.6-5.6-5.6-5.6L9 5Z",
  up: "m5 15 7-7 7 7-1.4 1.4-5.6-5.6-5.6 5.6L5 15Z",
  down: "m5 9 7 7 7-7-1.4-1.4-5.6 5.6-5.6-5.6L5 9Z",
  sort: "m12 5 5 6H7l5-6Z",
  close: "m6.4 5 5.6 5.6L17.6 5 19 6.4 13.4 12l5.6 5.6-1.4 1.4-5.6-5.6L6.4 19 5 17.6l5.6-5.6L5 6.4 6.4 5Z",
  more: "M6 10.5a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3Zm6 0a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3Zm6 0a1.5 1.5 0 1 1 0 3 1.5 1.5 0 0 1 0-3Z",
  search: "m20.3 21.7-5.4-5.4a7 7 0 1 1 1.4-1.4l5.4 5.4-1.4 1.4ZM10.5 15.5a5 5 0 1 0 0-10 5 5 0 0 0 0 10Z",
  check: "m9.5 16.2-4.2-4.2-1.4 1.4 5.6 5.6L21.1 7.4 19.7 6 9.5 16.2Z",
  outline: "M4 6h2v2H4V6Zm4 0h12v2H8V6Zm-4 5h2v2H4v-2Zm4 0h12v2H8v-2Zm-4 5h2v2H4v-2Zm4 0h12v2H8v-2Z",
  edit: "M4 17.2V20h2.8l8.3-8.3-2.8-2.8L4 17.2Zm13.7-7.5a.9.9 0 0 0 0-1.3l-2-2a.9.9 0 0 0-1.3 0L12.8 8l2.8 2.8 2.1-1.1Z",
  info: "M11 7h2v2h-2V7Zm0 4h2v6h-2v-6Zm1-9a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm0 18a8 8 0 1 1 0-16 8 8 0 0 1 0 16Z",
  warn: "M1 21h22L12 2 1 21Zm12-3h-2v-2h2v2Zm0-4h-2v-4h2v4Z",
  error: "M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20Zm1 15h-2v-2h2v2Zm0-4h-2V7h2v6Z",
  translate:
    "m12.9 15.1-2.5-2.5v-.1A17.4 17.4 0 0 0 14.1 6H17V4h-7V2H8v2H1v2h11.2a15.7 15.7 0 0 1-3.2 5.4 15.6 15.6 0 0 1-2.3-3.4h-2a17.6 17.6 0 0 0 3 4.6L2.6 17.6 4 19l5-5 3.1 3.1.8-2ZM18.5 10h-2L12 22h2l1.1-3h4.8l1.1 3h2l-4.5-12Zm-2.6 7 1.6-4.3 1.6 4.3h-3.2Z",
  export: "M5 20h14v-2H5v2Zm14-9h-4V3H9v8H5l7 7 7-7Z",
  convert: "M12 4V1L8 5l4 4V6a6 6 0 0 1 5.7 7.9l1.5 1.5A8 8 0 0 0 12 4Zm0 14a6 6 0 0 1-5.7-7.9L4.8 8.6A8 8 0 0 0 12 20v3l4-4-4-4v3Z",
  review: "M9 16.2 4.8 12l-1.4 1.4L9 19 21 7l-1.4-1.4L9 16.2ZM3 4h12v2H3V4Zm0 4h8v2H3V8Z",
  folder: "M10 4H4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-8l-2-2Zm-6 4h16v10H4V8Z",
  trash: "M9 3v1H4v2h1v13a2 2 0 0 0 2 2h10a2 2 0 0 0 2-2V6h1V4h-5V3H9Zm-2 3h10v13H7V6Zm2 2v9h2V8H9Zm4 0v9h2V8h-2Z",
  split: "M3 4h8v16H3V4Zm2 2v12h4V6H5Zm8-2h8v16h-8V4Zm2 4v2h4V8h-4Zm0 4v2h4v-2h-4Z",
  text: "M4 5h16v2H4V5Zm0 4h16v2H4V9Zm0 4h16v2H4v-2Zm0 4h10v2H4v-2Z",
  fit: "M4 4h6v2H6v4H4V4Zm10 0h6v6h-2V6h-4V4ZM4 14h2v4h4v2H4v-6Zm14 0h2v6h-6v-2h4v-4Z",
  file: "M6 2h8l6 6v12a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2Zm7 1.5V9h5.5L13 3.5Z",
  page: "M6 2h12a2 2 0 0 1 2 2v16a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2Zm1 4v2h10V6H7Zm0 4v2h10v-2H7Zm0 4v2h7v-2H7Z",
};

export function icon(name: string, extra = ""): SVGSVGElement {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("viewBox", "0 0 24 24");
  svg.setAttribute("class", `icon ${extra}`.trim());
  svg.setAttribute("aria-hidden", "true");
  const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
  path.setAttribute("d", ICONS[name] ?? "");
  svg.append(path);
  return svg;
}

/** Sustituye los `<i data-icon="…">` del HTML estático por su SVG. */
export function hydrateIcons(root: ParentNode = document): void {
  for (const node of root.querySelectorAll<HTMLElement>("i[data-icon]")) {
    node.replaceWith(icon(node.dataset.icon ?? "", node.className));
  }
}

// --- Confirmación -----------------------------------------------------------------------------

/** Pide confirmación para algo que no se puede deshacer. */
export function confirmAction(title: string, text: string, confirmLabel: string): Promise<boolean> {
  const dialog = $<HTMLDialogElement>("confirm");
  $("confirm-title").textContent = title;
  $("confirm-text").textContent = text;
  $("confirm-ok").textContent = confirmLabel;
  return new Promise((resolve) => {
    const done = (value: boolean) => {
      dialog.removeEventListener("close", onClose);
      $("confirm-ok").removeEventListener("click", onOk);
      $("confirm-cancel").removeEventListener("click", onCancel);
      if (dialog.open) dialog.close();
      resolve(value);
    };
    const onOk = () => done(true);
    const onCancel = () => done(false);
    const onClose = () => done(false);
    $("confirm-ok").addEventListener("click", onOk);
    $("confirm-cancel").addEventListener("click", onCancel);
    dialog.addEventListener("close", onClose);
    dialog.showModal();
  });
}

// --- Notificaciones ---------------------------------------------------------------------------

export type ToastKind = "info" | "success" | "warning" | "error";

export interface HistoryEntry {
  kind: ToastKind;
  message: string;
  time: Date;
}

export const history: HistoryEntry[] = [];
const historyListeners: (() => void)[] = [];

export function onHistory(listener: () => void): void {
  historyListeners.push(listener);
}

export const ICON_OF: Record<ToastKind, string> = { success: "check", error: "error", warning: "warn", info: "info" };

/**
 * Muestra un aviso abajo a la derecha. Los errores se quedan hasta cerrarlos; el resto se va solo
 * (y no se va mientras el ratón está encima). `action` añade un botón, por ejemplo «Abrir».
 */
export function toast(message: string, kind: ToastKind = "info", action?: { label: string; run: () => void }): void {
  const region = $("toasts");
  let timer = 0;
  const dismiss = () => {
    window.clearTimeout(timer);
    node.classList.add("leaving");
    window.setTimeout(() => node.remove(), 200);
  };
  const node = h(
    "div",
    { class: `toast ${kind}`, role: kind === "error" ? "alert" : "status" },
    icon(ICON_OF[kind], "toast-icon"),
    h("p", { text: message }),
    action
      ? button(action.label, () => {
          action.run();
          dismiss();
        }, { class: "toast-action" })
      : null,
    button(icon("close"), () => dismiss(), { class: "icon-button toast-close", "aria-label": "Cerrar aviso" }),
  );
  const schedule = () => {
    if (kind === "error") return;
    timer = window.setTimeout(dismiss, kind === "success" || action ? 8000 : 5000);
  };
  node.addEventListener("mouseenter", () => window.clearTimeout(timer));
  node.addEventListener("mouseleave", schedule);
  region.append(node);
  while (region.children.length > 4) region.firstElementChild?.remove();
  schedule();
  history.unshift({ kind, message, time: new Date() });
  history.length = Math.min(history.length, 40);
  for (const listener of historyListeners) listener();
}

export function errorToast(error: unknown): void {
  toast(String(error), "error");
}
