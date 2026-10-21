// Tipos y órdenes del backend. Toda la comunicación con Rust pasa por aquí.
import { invoke } from "@tauri-apps/api/core";

export interface TranslationSummary {
  language: string;
  label: string;
  quality: boolean;
  blocks: number;
  doubts: number;
  seconds: number;
  untouched: number;
}

export interface DocumentInfo {
  id: string;
  name: string;
  /** Ruta del original. */
  path: string;
  pages: number;
  available: boolean;
  converted: boolean;
  converting: boolean;
  resumable: boolean;
  visual_full: boolean;
  paged: boolean;
  translating: boolean;
  translations: TranslationSummary[];
  /** `null` si la lista de avisos todavía no se ha calculado. */
  review_total: number | null;
  review_pending: number;
}

export interface TranslationEvent {
  id: string;
  state: "running" | "done" | "failed" | "cancelled";
  done: number;
  total: number;
  message: string | null;
  summary: TranslationSummary | null;
  epub: boolean;
}

export interface TranslationOptions {
  languages: [string, string][];
  fast: boolean;
  quality: boolean;
}

export interface TranslationDoubt {
  page: number;
  block: number;
  reason: string;
  excerpt: string;
}

export interface ComponentInfo {
  id: string;
  name: string;
  description: string;
  license: string;
  required: boolean;
  megabytes: number;
  installed: boolean;
  installing: boolean;
}

export interface ComponentEvent {
  id: string;
  state: "running" | "done" | "failed" | "cancelled";
  done: number;
  total: number;
  message: string | null;
}

export interface EpubBlock {
  index: number;
  tag: string;
  text: string;
  editable: boolean;
}

export interface EpubChapter {
  chapters: number;
  chapter: number;
  blocks: EpubBlock[];
  edited: boolean;
}

export interface ReviewEntry {
  page: number;
  block: number | null;
  key: string;
  excerpt: string;
  risk: number;
  reasons: string[];
  decision: string | null;
}

export interface ReviewView {
  items: ReviewEntry[];
  total_risk: number;
  pending_risk: number;
}

export interface PageView {
  page: number;
  html: string;
}

export interface Summary {
  seconds: number;
  removed_margins: number;
  joined_words: number;
  headings: number;
  images: number;
  scanned: boolean;
  doubtful_pages: number;
  reread_pages: number;
  visual_available: boolean;
}

export interface ConversionEvent {
  id: string;
  state: "running" | "done" | "failed" | "cancelled";
  message: string | null;
  summary: Summary | null;
}

export const api = {
  listDocuments: () => invoke<DocumentInfo[]>("list_documents"),
  openDocument: (path: string) => invoke<DocumentInfo>("open_document", { path }),
  removeDocument: (id: string) => invoke<void>("remove_document", { id }),
  renderPage: (id: string, page: number, width: number) =>
    invoke<ArrayBuffer>("render_page", { id, page, width }),
  convert: (id: string) => invoke<void>("convert_document", { id }),
  cancelConversion: (id: string) => invoke<boolean>("cancel_conversion", { id }),
  setVisualFull: (id: string, on: boolean) => invoke<void>("set_visual_full", { id, on }),
  resultPages: (id: string, translation: string | null) =>
    invoke<PageView[]>("result_pages", { id, translation }),
  pageMarkdown: (id: string, page: number) => invoke<string>("page_markdown", { id, page }),
  savePage: (id: string, page: number, text: string) =>
    invoke<void>("save_page", { id, page, text }),
  reviewList: (id: string) => invoke<ReviewView>("review_list", { id }),
  reviewBlockText: (id: string, page: number, block: number) =>
    invoke<string>("review_block_text", { id, page, block }),
  reviewDecide: (id: string, item: ReviewEntry, decision: string, text: string | null) =>
    invoke<ReviewView>("review_decide", {
      id,
      key: item.key,
      decision,
      page: item.page,
      block: item.block,
      text,
    }),
  translationOptions: () => invoke<TranslationOptions>("translation_options"),
  translate: (
    id: string,
    language: string,
    quality: boolean,
    glossary: string,
    epub: boolean,
  ) => invoke<void>("translate_document", { id, language, quality, glossary, epub }),
  cancelTranslation: (id: string) => invoke<boolean>("cancel_translation", { id }),
  translationDoubts: (id: string, language: string) =>
    invoke<TranslationDoubt[]>("translation_doubts", { id, language }),
  exportMarkdown: (
    id: string,
    translation: string | null,
    pages: [number, number] | null,
    path: string,
  ) => invoke<void>("export_markdown", { id, translation, pages, path }),
  exportEpub: (args: {
    id: string;
    translation: string | null;
    pages: [number, number] | null;
    cover: boolean;
    path: string;
    title: string;
    author: string | null;
    language: string;
  }) => invoke<void>("export_epub", args),
  openInEpubEditor: (path: string) => invoke<string>("open_in_epub_editor", { path }),
  epubChapter: (id: string, chapter: number) =>
    invoke<EpubChapter>("epub_chapter", { id, chapter }),
  epubSetText: (id: string, chapter: number, index: number, text: string) =>
    invoke<EpubChapter>("epub_set_text", { id, chapter, index, text }),
  epubSaveAs: (id: string, path: string) => invoke<void>("epub_save_as", { id, path }),
  epubDiscard: (id: string) => invoke<void>("epub_discard", { id }),
  previewMarkdown: (id: string, text: string) => invoke<string>("preview_markdown", { id, text }),
  revealPath: (path: string) => invoke<void>("reveal_path", { path }),
  revealData: () => invoke<void>("reveal_data"),
  listComponents: () => invoke<ComponentInfo[]>("list_components"),
  installComponent: (id: string) => invoke<void>("install_component", { id }),
  cancelComponent: (id: string) => invoke<boolean>("cancel_component", { id }),
};
