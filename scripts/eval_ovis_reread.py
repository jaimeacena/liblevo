"""Relee con OvisOCR2 las páginas dudosas de una salida de Liblevo y escribe otra candidata (F3).

Herramienta de desarrollo para decidir la integración: toma `ENTRADA/<id>/document.md` y las
páginas dudosas que la conversión dejó en `run.json`, relee cada una con `llama-mtmd-cli` de
llama.cpp y sustituye su texto. Escribe `SALIDA/<id>/document.md` y `run.json` con el tiempo de la
conversión más el de la relectura. Sin conexión; no registra contenido documental.
"""

from __future__ import annotations

import argparse
import json
import re
import shutil
import tempfile
from pathlib import Path

import pypdfium2 as pdfium

from eval_vlm_pages import PROMPT, run_measured_to

MARKER = re.compile(r"<!-- page: (\d+) -->")


def split_pages(markdown: str) -> tuple[str, dict[int, str]]:
    """Texto antes de la primera marca y texto de cada página."""
    parts = MARKER.split(markdown)
    pages = {int(parts[i]): parts[i + 1] for i in range(1, len(parts) - 1, 2)}
    return parts[0], pages


def join_pages(head: str, pages: dict[int, str]) -> str:
    return head + "".join(f"<!-- page: {n} -->{text}" for n, text in sorted(pages.items()))


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--mmproj", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, default=Path("eval/manifest.json"))
    parser.add_argument("--pdf-dir", type=Path, required=True)
    parser.add_argument("--input", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--scale", type=float, default=2.0)
    args = parser.parse_args()

    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    for document in manifest["documents"]:
        source = args.input / document["id"]
        if not (source / "run.json").exists():
            continue
        run = json.loads((source / "run.json").read_text(encoding="utf-8"))
        target = args.out / document["id"]
        if target.exists():
            shutil.rmtree(target)
        shutil.copytree(source, target)
        doubtful = run.get("doubtful_pages") or []
        head, pages = split_pages((source / "document.md").read_text(encoding="utf-8"))
        pdf = pdfium.PdfDocument(str(args.pdf_dir / f"{document['id']}.pdf"))
        reread_seconds, failures = 0.0, 0
        for number in doubtful:
            print(f"{document['id']} p{number}…", flush=True)
            with tempfile.TemporaryDirectory() as temporary:
                image = Path(temporary) / "page.png"
                output = Path(temporary) / "out.txt"
                pdf[number - 1].render(scale=args.scale).to_pil().save(image)
                command = [
                    str(args.cli), "-m", str(args.model), "--mmproj", str(args.mmproj),
                    "--image", str(image), "-p", PROMPT, "-c", "12288", "-n", "4096",
                    "--temp", "0", "--repeat-penalty", "1.1", "--repeat-last-n", "64",
                    "-t", "4", "-fa", "on",
                ]  # fmt: skip
                with output.open("w", encoding="utf-8") as sink:
                    code, seconds, _ = run_measured_to(command, sink)
                reread_seconds += seconds
                text = output.read_text(encoding="utf-8", errors="replace")
                text = re.sub(r"<think>.*?</think>", "", text, flags=re.S).strip()
                if code != 0 or not text:
                    failures += 1
                    continue
                pages[number] = f"\n\n{text}\n\n"
        (target / "document.md").write_text(join_pages(head, pages), encoding="utf-8")
        run["seconds"] = round(float(run.get("seconds", 0)) + reread_seconds, 2)
        run["reread_pages"] = len(doubtful) - failures
        run["reread_seconds"] = round(reread_seconds, 2)
        if failures:
            run["note"] = f"{failures} relecturas fallidas; se conserva el OCR"
        (target / "run.json").write_text(json.dumps(run, ensure_ascii=False), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
