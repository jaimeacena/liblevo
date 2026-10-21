"""Lee con un modelo visual local solo las páginas de referencia de los escaneos (F1).

Usa `llama-mtmd-cli` de llama.cpp con OvisOCR2 y el mismo prompt del prototipo Rust. Escribe
`SALIDA/<id>/document.md` con marcas `<!-- page: N -->` y `run.json` con el tiempo total, la
memoria máxima y los segundos por página. Herramienta temporal de la F1; sin conexión.
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import tempfile
from pathlib import Path

import pypdfium2 as pdfium

PROMPT = (
    "Extract all readable content from the image in natural human reading order and output the "
    "result as a single Markdown document. For charts or images, represent them using an HTML "
    'image tag: <img src="images/bbox_{left}_{top}_{right}_{bottom}.jpg" />, where left, top, '
    "right, bottom are bounding box coordinates scaled to [0, 1000). Format formulas as LaTeX. "
    "Format tables as HTML: <table>...</table>. Transcribe all other text as standard Markdown. "
    "Preserve the original text without translation or paraphrasing."
)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--cli", type=Path, required=True)
    parser.add_argument("--model", type=Path, required=True)
    parser.add_argument("--mmproj", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, default=Path("eval/manifest.json"))
    parser.add_argument("--pdf-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--scale", type=float, default=2.0)
    args = parser.parse_args()

    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    for document in manifest["documents"]:
        pages = document.get("reference_pages") or []
        if document["kind"] != "scanned" or not pages:
            continue
        target = args.out / document["id"]
        target.mkdir(parents=True, exist_ok=True)
        source = pdfium.PdfDocument(str(args.pdf_dir / f"{document['id']}.pdf"))
        parts: list[str] = []
        total_seconds, peak, failures = 0.0, 0.0, 0
        for number in pages:
            print(f"{document['id']} p{number}…", flush=True)
            with tempfile.TemporaryDirectory() as temporary:
                image = Path(temporary) / "page.png"
                output = Path(temporary) / "out.txt"
                source[number - 1].render(scale=args.scale).to_pil().save(image)
                command = [
                    str(args.cli), "-m", str(args.model), "--mmproj", str(args.mmproj),
                    "--image", str(image), "-p", PROMPT, "-c", "12288", "-n", "4096",
                    "--temp", "0", "--repeat-penalty", "1.1", "--repeat-last-n", "64",
                    "-t", "4", "-fa", "on",
                ]  # fmt: skip
                with output.open("w", encoding="utf-8") as sink:
                    code, seconds, memory = run_measured_to(command, sink)
                total_seconds += seconds
                peak = max(peak, memory)
                text = output.read_text(encoding="utf-8", errors="replace")
                text = re.sub(r"<think>.*?</think>", "", text, flags=re.S).strip()
                if code != 0 or not text:
                    failures += 1
                parts.append(f"<!-- page: {number} -->\n{text}\n")
        (target / "document.md").write_text("\n".join(parts), encoding="utf-8")
        run = {
            "status": "ok" if failures == 0 else "failed",
            "seconds": round(total_seconds, 2),
            "peak_mib": round(peak, 1),
            "note": f"solo páginas de referencia; {total_seconds / len(pages):.0f} s/página",
        }
        (target / "run.json").write_text(json.dumps(run, ensure_ascii=False), encoding="utf-8")
    return 0


def run_measured_to(command: list[str], sink) -> tuple[int, float, float]:  # noqa: ANN001
    """Ejecuta el modelo guardando su salida estándar; mide tiempo y memoria máxima."""
    import threading
    import time

    import psutil

    started = time.perf_counter()
    child = psutil.Popen(command, stdout=sink, stderr=subprocess.DEVNULL)
    peak = 0.0
    stop = threading.Event()

    def sample() -> None:
        nonlocal peak
        while not stop.is_set():
            try:
                peak = max(peak, child.memory_info().rss / 2**20)
            except psutil.Error:
                pass
            stop.wait(0.2)

    thread = threading.Thread(target=sample, daemon=True)
    thread.start()
    code = child.wait(timeout=1800)
    stop.set()
    thread.join()
    return code, time.perf_counter() - started, peak


if __name__ == "__main__":
    raise SystemExit(main())
