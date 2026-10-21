// Traducir: nueva traducción (idioma, modelo y glosario), progreso, guardadas y avisos.
import { api } from "./api";
import { $, button, errorToast, frag, h, icon, toast } from "./dom";
import { setLayout } from "./read";
import { actions, duration, isEpub, plural, state, stem } from "./state";

/** Segundos por página de texto corrido, medidos con el conjunto (unas 300 palabras). */
const SECONDS_PER_PAGE = { fast: 30, quality: 93 };

function model(): "fast" | "quality" {
  const checked = document.querySelector<HTMLInputElement>('input[name="translate-model"]:checked');
  return checked?.value === "quality" ? "quality" : "fast";
}

function glossaryTerms(): number {
  return $<HTMLTextAreaElement>("translate-glossary")
    .value.split("\n")
    .filter((line) => line.includes("=") && line.split("=").every((part) => part.trim())).length;
}

export async function prepareTranslate(): Promise<void> {
  const doc = state.current;
  if (!doc) return;
  renderTranslations();
  renderProgress();
  try {
    const options = await api.translationOptions();
    const available = options.fast || options.quality;
    $("translate-missing").hidden = available;
    $<HTMLButtonElement>("translate-start").disabled = !available || doc.translating;
    const language = $<HTMLSelectElement>("translate-language");
    if (!language.options.length) {
      language.replaceChildren(...options.languages.map(([code, label]) => new Option(label, code)));
      try {
        const saved = localStorage.getItem("liblevo.translateTo");
        if (saved && options.languages.some(([code]) => code === saved)) language.value = saved;
      } catch {
        // Sin preferencia guardada, el primer idioma.
      }
    }
    for (const card of document.querySelectorAll<HTMLLabelElement>(".model-card")) {
      const which = card.dataset.model as "fast" | "quality";
      const ready = which === "fast" ? options.fast : options.quality;
      const input = card.querySelector("input") as HTMLInputElement;
      input.disabled = !ready;
      const status = card.querySelector(".model-state") as HTMLElement;
      status.replaceChildren(
        ready
          ? h("span", { class: "pill ok" }, icon("check"), "Instalado")
          : button(which === "fast" ? "Descargar «Traducción»" : "Descargar «Más calidad»", (event) => {
              event.preventDefault();
              actions.openComponents();
            }, { class: "link" }),
      );
    }
    const current = document.querySelector<HTMLInputElement>('input[name="translate-model"]:checked');
    if (current?.disabled) {
      const other = document.querySelector<HTMLInputElement>('input[name="translate-model"]:not(:disabled)');
      if (other) other.checked = true;
    }
    const epub = $<HTMLInputElement>("translate-epub");
    $("translate-epub-option").hidden = !isEpub(doc);
    if (!doc.converted) epub.checked = isEpub(doc);
    epub.disabled = isEpub(doc) && !doc.converted;
    renderEstimate();
  } catch (error) {
    errorToast(error);
  }
}

function renderEstimate(): void {
  const doc = state.current;
  const note = $("translate-estimate");
  const terms = glossaryTerms();
  $("glossary-count").textContent = plural(terms, "término", "términos");
  if (!doc?.paged) {
    note.textContent = "Lo ya traducido se guarda: si cancelas o cierras la app, se reanuda donde se quedó.";
    return;
  }
  const seconds = doc.pages * SECONDS_PER_PAGE[model()];
  note.textContent = `Unos ${duration(seconds)} para ${plural(doc.pages, "página", "páginas")} de texto corrido. Si cancelas o cierras la app, se reanuda donde se quedó.`;
}

export function renderProgress(): void {
  const doc = state.current;
  if (!doc) return;
  const card = $("translate-progress");
  card.hidden = !doc.translating;
  $("translate-form").hidden = doc.translating;
  if (!doc.translating) return;
  const [done, total] = state.translationProgress.get(doc.id) ?? [0, 0];
  const bar = $<HTMLProgressElement>("translate-bar");
  $("translate-progress-title").textContent = stem(doc);
  if (total) {
    bar.value = done / total;
    $("translate-percent").textContent = `${Math.round((100 * done) / total)} %`;
  } else {
    bar.removeAttribute("value");
    $("translate-percent").textContent = "";
  }
  $("translate-progress-text").textContent = total
    ? `${done.toLocaleString("es-ES")} de ${plural(total, "bloque", "bloques")}. Puedes seguir trabajando; te avisaré al terminar.`
    : "Comprobando el modelo y preparando el texto…";
}

export function renderTranslations(): void {
  const doc = state.current;
  if (!doc) return;
  $("translations-empty").hidden = doc.translations.length > 0;
  $("translations").replaceChildren(
    ...doc.translations.map((t) =>
      h(
        "li",
        {},
        h("span", { class: "lang", text: t.language.toUpperCase() }),
        h(
          "div",
          { class: "info" },
          h("strong", { text: t.label }),
          h("span", {
            class: "subtle",
            text: `${t.quality ? "Más calidad" : "Rápido"} · ${plural(t.blocks, "bloque", "bloques")} · ${duration(t.seconds)}`,
          }),
        ),
        t.doubts
          ? button(plural(t.doubts, "aviso", "avisos"), () => void loadDoubts(t.language), { class: "ghost" })
          : h("span", { class: "pill ok" }, icon("check"), "Sin avisos"),
        button("Comparar", async () => {
          state.viewing = t.language;
          await setLayout("compare");
          actions.setTab("leer");
        }),
        button("Leer", () => {
          state.viewing = t.language;
          actions.setTab("leer");
        }, { class: "primary" }),
      ),
    ),
  );
}

async function loadDoubts(language: string): Promise<void> {
  const doc = state.current;
  const info = doc?.translations.find((t) => t.language === language);
  if (!doc || !info) return;
  try {
    const doubts = await api.translationDoubts(doc.id, language);
    $("translation-review").hidden = false;
    $("translation-review-title").textContent = `Avisos · ${info.label}`;
    $("translation-summary").textContent = doubts.length
      ? `${plural(doubts.length, "bloque", "bloques")} para revisar. El resto no tiene avisos, pero conviene leerlo.`
      : "Sin avisos; conviene leerla igualmente.";
    $("translation-doubts").replaceChildren(
      ...doubts.map((doubt) =>
        h(
          "li",
          {},
          h(
            "div",
            { class: "row spread" },
            h("strong", { text: doubt.reason }),
            button(frag("Ver en la página ", String(doubt.page)), () => {
              state.viewing = language;
              void actions.goTo(doubt.page, doubt.block);
            }, { class: "ghost" }),
          ),
          h("p", { class: "excerpt", text: doubt.excerpt }),
        ),
      ),
    );
    $("translation-review").scrollIntoView({ behavior: "smooth", block: "start" });
  } catch (error) {
    errorToast(error);
  }
}

async function start(event: SubmitEvent): Promise<void> {
  event.preventDefault();
  const doc = state.current;
  if (!doc) return;
  const language = $<HTMLSelectElement>("translate-language").value;
  try {
    await api.translate(
      doc.id,
      language,
      model() === "quality",
      $<HTMLTextAreaElement>("translate-glossary").value,
      $<HTMLInputElement>("translate-epub").checked,
    );
    try {
      localStorage.setItem("liblevo.translateTo", language);
    } catch {
      // Recordar el idioma es solo una comodidad.
    }
    state.translationProgress.set(doc.id, [0, 0]);
    toast("Traducción en marcha. Puedes seguir trabajando; te avisaré al terminar.");
    await actions.refresh(doc.id);
  } catch (error) {
    errorToast(error);
  }
}

export function initTranslate(): void {
  $("translate-form").addEventListener("submit", (event) => void start(event as SubmitEvent));
  for (const input of document.querySelectorAll<HTMLInputElement>('input[name="translate-model"]')) {
    input.addEventListener("change", renderEstimate);
  }
  $("translate-glossary").addEventListener("input", renderEstimate);
  $("translate-cancel").addEventListener("click", () => {
    if (state.current) void api.cancelTranslation(state.current.id);
  });
  $("translate-get").addEventListener("click", () => actions.openComponents());
  $("translation-review-close").addEventListener("click", () => ($("translation-review").hidden = true));
}
