"""Mide la traducción (F6): estructura intacta, tiempo y avisos, sin escribir contenido documental.

Para cada documento traduce el resultado convertido (o sus primeros bloques) con
`liblevo-traducir` y compara original y traducción: marcas de página, número de bloques, niveles
de título, elementos de lista, forma de las tablas (filas y columnas), imágenes, bloques de código
y destinos de enlaces deben coincidir exactamente. También cuenta los números (cifras) que se
pierden y los bloques idénticos al original. Escribe solo recuentos y tiempos.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import subprocess
import time
from pathlib import Path

PAGE = re.compile(r"<!-- page: (\d+) -->")
LINK = re.compile(r"\]\(([^)]*)\)")
NUMBER = re.compile(r"\d+(?:[.,]\d+)*")


def blocks(markdown: str) -> list[str]:
    """Bloques separados por línea en blanco, con los bloques de código enteros."""
    out: list[str] = []
    current: list[str] = []
    fenced = False
    for line in markdown.replace("\r\n", "\n").split("\n"):
        if line.lstrip().startswith("```"):
            fenced = not fenced
        if not line.strip() and not fenced:
            if current:
                out.append("\n".join(current))
                current = []
            continue
        current.append(line)
    if current:
        out.append("\n".join(current))
    return out


def shape(block: str) -> str:
    """Forma estructural de un bloque, sin su texto."""
    text = block.strip()
    lines = text.split("\n")
    if PAGE.fullmatch(text):
        return text
    if text.startswith("```") or text.startswith("$$") or text.startswith("<!--"):
        return "literal:" + text
    if text.startswith("!["):
        return "imagen:" + text
    if all(line.lstrip().startswith("|") for line in lines):
        columns = [len(line.strip().strip("|").split("|")) for line in lines]
        return f"tabla:{len(lines)}x{columns}"
    heading = re.match(r"(#{1,6}) ", text)
    if heading:
        return f"título:{len(heading.group(1))}"
    items = [re.match(r"\s*(- |\* |> |\d+\. )", line) for line in lines]
    if len(lines) > 1 and all(items):
        return "lista:" + ",".join(m.group(1).strip() for m in items if m)
    return "párrafo"


def numbers(text: str) -> list[str]:
    return sorted(n.replace(",", ".") for n in NUMBER.findall(text))


def compare(source: str, translated: str) -> dict[str, object]:
    a, b = blocks(source), blocks(translated)
    shapes_a, shapes_b = [shape(x) for x in a], [shape(x) for x in b]
    differing = sum(1 for x, y in zip(shapes_a, shapes_b, strict=False) if x != y)
    mismatched = differing + abs(len(a) - len(b))
    links_a = [LINK.findall(x) for x in a]
    links_b = [LINK.findall(x) for x in b]
    text_pairs = [
        (x, y)
        for x, y, s in zip(a, b, shapes_a, strict=False)
        if s in ("párrafo",) or s.startswith(("título", "lista", "tabla"))
    ]
    lost_numbers = 0
    total_numbers = 0
    for x, y in text_pairs:
        nx, ny = numbers(x), numbers(y)
        total_numbers += len(nx)
        remaining = list(ny)
        for n in nx:
            if n in remaining:
                remaining.remove(n)
            else:
                lost_numbers += 1
    identical = sum(1 for x, y in text_pairs if x.strip() == y.strip() and len(x.split()) >= 5)
    return {
        "bloques": len(a),
        "bloques_traducidos": len(b),
        "bloques_con_otra_estructura": mismatched,
        "paginas_iguales": PAGE.findall(source) == PAGE.findall(translated),
        "enlaces_iguales": links_a == links_b,
        "cifras": total_numbers,
        "cifras_perdidas": lost_numbers,
        "bloques_sin_traducir": identical,
        "palabras": len(source.split()),
    }


def first_blocks(markdown: str, limit: int) -> str:
    """Igual que `LIBLEVO_MAX_BLOQUES` en `liblevo-traducir`."""
    out, count = [], 0
    for block in markdown.replace("\r\n", "\n").split("\n\n"):
        text = block.strip()
        if text and not text.startswith(("<!--", "![")):
            if count == limit:
                break
            count += 1
        out.append(block)
    return "\n\n".join(out)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--programa", required=True, type=Path, help="liblevo-traducir.exe")
    parser.add_argument("--llama", required=True, type=Path, help="carpeta de llama.cpp")
    parser.add_argument("--modelo", required=True, type=Path)
    parser.add_argument("--salidas", required=True, type=Path, help="resultados convertidos")
    parser.add_argument("--destino", required=True, type=Path, help="carpeta para traducciones")
    parser.add_argument("--json", required=True, type=Path)
    parser.add_argument(
        "--caso",
        action="append",
        required=True,
        help="documento:idioma[:bloques], p. ej. d1-gift-of-the-magi:es:40",
    )
    args = parser.parse_args()
    results = []
    for case in args.caso:
        name, language, *limit = case.split(":")
        source_path = args.salidas / name / "document.md"
        source = source_path.read_text(encoding="utf-8")
        allowed = ("SYSTEMROOT", "PATH", "TEMP", "TMP")
        env = {k: v for k, v in os.environ.items() if k.upper() in allowed}
        if limit:
            env["LIBLEVO_MAX_BLOQUES"] = limit[0]
            source = first_blocks(source, int(limit[0]))
        output = args.destino / f"{name}.{language}.md"
        output.parent.mkdir(parents=True, exist_ok=True)
        started = time.monotonic()
        run = subprocess.run(
            [
                str(args.programa),
                str(args.llama),
                str(args.modelo),
                str(source_path),
                language,
                str(output),
            ],
            env=env,
            capture_output=True,
            text=True,
            encoding="utf-8",
            errors="replace",
        )
        seconds = time.monotonic() - started
        entry: dict[str, object] = {
            "documento": name,
            "idioma": language,
            "segundos": round(seconds, 1),
        }
        if run.returncode != 0:
            entry["error"] = run.stderr.strip().splitlines()[-1:] or ["sin mensaje"]
        else:
            report = json.loads(output.with_suffix(".json").read_text(encoding="utf-8"))
            entry.update(compare(source, output.read_text(encoding="utf-8")))
            kinds: dict[str, int] = {}
            for _page, _block, kind in report["doubts"]:
                kinds[kind] = kinds.get(kind, 0) + 1
            entry["avisos"] = kinds
            entry["reutilizados"] = report["reused"]
            words = int(entry["palabras"])  # type: ignore[call-overload]
            entry["segundos_por_100_palabras"] = round(100 * report["seconds"] / max(words, 1), 1)
        results.append(entry)
        print(json.dumps(entry, ensure_ascii=False))
    args.json.parent.mkdir(parents=True, exist_ok=True)
    summary = {"modelo": args.modelo.name, "casos": results}
    args.json.write_text(
        json.dumps(summary, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )


if __name__ == "__main__":
    main()
