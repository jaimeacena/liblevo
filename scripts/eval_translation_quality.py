"""Mide la calidad de la traducción frente a traducciones de referencia, sin escribir contenido.

Las referencias viven fuera del repositorio (un JSON por idioma con, para cada documento, el texto
de referencia de algunos bloques, numerados por su posición al separar por línea en blanco). Para
cada traducción candidata (`etiqueta=carpeta`, con `<documento>.<idioma>.md` dentro) calcula chrF
(n-gramas de 6 letras, beta 2, sin espacios), la medida habitual para comparar traducciones de un
mismo texto: 100 es idéntica a la referencia. Solo sirve para comparar candidatos entre sí sobre
las mismas referencias; no es una nota absoluta de calidad.
"""

from __future__ import annotations

import argparse
import json
from collections import Counter
from pathlib import Path

ORDER = 6
BETA = 2.0


def ngrams(text: str, n: int) -> Counter[str]:
    chars = "".join(text.split())
    return Counter(chars[i : i + n] for i in range(len(chars) - n + 1))


def chrf(pairs: list[tuple[str, str]]) -> float:
    """chrF de corpus: estadísticas sumadas sobre todos los pares (candidato, referencia)."""
    precisions, recalls = [], []
    for n in range(1, ORDER + 1):
        matched = total_candidate = total_reference = 0
        for candidate, reference in pairs:
            c, r = ngrams(candidate, n), ngrams(reference, n)
            matched += sum((c & r).values())
            total_candidate += sum(c.values())
            total_reference += sum(r.values())
        if total_candidate and total_reference:
            precisions.append(matched / total_candidate)
            recalls.append(matched / total_reference)
    if not precisions:
        return 0.0
    p = sum(precisions) / len(precisions)
    r = sum(recalls) / len(recalls)
    if p + r == 0:
        return 0.0
    return 100 * (1 + BETA**2) * p * r / (BETA**2 * p + r)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--referencias", required=True, type=Path, help="JSON de un idioma")
    parser.add_argument("--idioma", required=True)
    parser.add_argument(
        "--candidato", action="append", required=True, help="etiqueta=carpeta de traducciones"
    )
    parser.add_argument("--json", required=True, type=Path)
    args = parser.parse_args()
    references = json.loads(args.referencias.read_text(encoding="utf-8"))["documentos"]
    results = []
    for spec in args.candidato:
        label, folder = spec.split("=", 1)
        entry: dict[str, object] = {"candidato": label, "documentos": {}}
        everything: list[tuple[str, str]] = []
        for document, blocks in references.items():
            path = Path(folder) / f"{document}.{args.idioma}.md"
            if not path.is_file():
                continue
            translated = path.read_text(encoding="utf-8").replace("\r\n", "\n").split("\n\n")
            pairs = [
                (translated[int(i)].strip(), text)
                for i, text in blocks.items()
                if int(i) < len(translated)
            ]
            everything += pairs
            entry["documentos"][document] = {  # type: ignore[index]
                "bloques": len(pairs),
                "chrf": round(chrf(pairs), 1),
            }
        entry["chrf_total"] = round(chrf(everything), 1)
        results.append(entry)
        print(json.dumps(entry, ensure_ascii=False))
    args.json.parent.mkdir(parents=True, exist_ok=True)
    args.json.write_text(
        json.dumps({"idioma": args.idioma, "candidatos": results}, ensure_ascii=False, indent=2)
        + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
