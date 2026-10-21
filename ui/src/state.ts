// Estado compartido de la interfaz y utilidades de presentación sin DOM.
import type { DocumentInfo } from "./api";

export type Tab = "leer" | "revisar" | "traducir" | "exportar" | "epub";
export type View = "library" | "doc";

export const SUPPORTED = ["pdf", "docx", "epub", "html", "htm", "txt", "md", "markdown"];

export const state = {
  documents: [] as DocumentInfo[],
  current: null as DocumentInfo | null,
  view: "library" as View,
  tab: "leer" as Tab,
  page: 1,
  /** Idioma de la traducción que se lee; `null` es el resultado. */
  viewing: null as string | null,
  /** Traducciones en curso: documento → [hechos, total]. */
  translationProgress: new Map<string, [number, number]>(),
  /** Descargas en curso: componente → fracción. */
  componentProgress: new Map<string, number>(),
  installing: new Set<string>(),
  /** Documento seleccionado en la biblioteca (el inspector lo resume). */
  selectedId: null as string | null,
};

export function extension(name: string): string {
  return name.split(".").pop()?.toLowerCase() ?? "";
}

export function formatOf(name: string): string {
  const ext = extension(name);
  return ext === "markdown" ? "md" : ext === "htm" ? "html" : ext;
}

export function isEpub(doc: DocumentInfo): boolean {
  return formatOf(doc.name) === "epub";
}

export function stem(doc: DocumentInfo): string {
  return doc.name.replace(/\.[a-z0-9]+$/i, "");
}

export function duration(seconds: number): string {
  if (seconds < 90) return `${Math.max(1, Math.round(seconds))} s`;
  if (seconds < 5400) return `${Math.round(seconds / 60)} min`;
  return `${(seconds / 3600).toFixed(1).replace(".", ",")} h`;
}

export function plural(n: number, one: string, many: string): string {
  return `${n.toLocaleString("es-ES")} ${n === 1 ? one : many}`;
}

/** Estado de un documento en una frase corta. */
export function stateLabel(doc: DocumentInfo): string {
  if (!doc.available) return "No se encuentra el original";
  if (doc.converting) return "Convirtiendo…";
  if (doc.translating) {
    const [done, total] = state.translationProgress.get(doc.id) ?? [0, 0];
    return total ? `Traduciendo · ${done} de ${total}` : "Preparando la traducción…";
  }
  if (doc.resumable) return "Conversión interrumpida";
  if (doc.converted) return "Convertido";
  return "Sin convertir";
}

export function sizeLabel(doc: DocumentInfo): string {
  return doc.paged ? plural(doc.pages, "página", "páginas") : formatOf(doc.name).toUpperCase();
}

/** Siguiente paso recomendado para un documento. */
export type NextStep = "convert" | "resume" | "cancel" | "review" | "export" | "none";

export function nextStep(doc: DocumentInfo): NextStep {
  if (!doc.available) return "none";
  if (doc.converting) return "cancel";
  if (doc.resumable) return "resume";
  if (!doc.converted) return "convert";
  if (doc.review_pending > 0) return "review";
  return "export";
}

/**
 * Acciones compartidas entre vistas. `main.ts` las conecta al arrancar; así las vistas no se
 * importan entre sí.
 */
export const actions = {
  refresh: async (_selectId?: string): Promise<void> => {},
  open: async (_doc: DocumentInfo, _page?: number): Promise<void> => {},
  showLibrary: (): void => {},
  setTab: (_tab: Tab): void => {},
  goTo: async (_page: number, _block: number | null): Promise<void> => {},
  reviewAt: (_page: number, _block: number | null): void => {},
  openComponents: (): void => {},
  addDocuments: async (): Promise<void> => {},
  convert: async (_doc: DocumentInfo): Promise<void> => {},
  remove: async (_doc: DocumentInfo): Promise<boolean> => false,
  openPalette: (): void => {},
  openTab: async (_doc: DocumentInfo, _tab: Tab): Promise<void> => {},
};
