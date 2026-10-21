"""Ejecuta un candidato de línea de comandos (Xberg o docling.rs) sobre el conjunto de evaluación.

Escribe `SALIDA/<id>/document.md` con marcas `<!-- page: N -->` y `SALIDA/<id>/run.json` con
tiempo y memoria máxima del proceso, el formato que mide `liblevo-eval`. Solo usa binarios y
modelos ya presentes en `--tools`; fija el modo sin conexión y CPU con cuatro hilos.
Herramienta temporal de la F1.
"""

from __future__ import annotations

import argparse
import json
import os
import re
import shutil
import subprocess
import tempfile
import threading
import time
from pathlib import Path

import psutil
import pypdfium2 as pdfium

THREADS = "4"
PAGE_BREAK = "<!-- liblevo-page-break -->"


def subset(pdf: Path, first: int, last: int, folder: Path) -> Path:
    """Copia solo las páginas medidas, para que todos los candidatos lean lo mismo."""
    source = pdfium.PdfDocument(str(pdf))
    target = pdfium.PdfDocument.new()
    target.import_pages(source, list(range(first - 1, last)))
    path = folder / "subset.pdf"
    target.save(str(path))
    return path


def environment(tools: Path) -> dict[str, str]:
    allowed = {
        "SYSTEMROOT",
        "WINDIR",
        "PATH",
        "PATHEXT",
        "TEMP",
        "TMP",
        "COMSPEC",
        "USERPROFILE",
        "LOCALAPPDATA",
        "APPDATA",
        "PROGRAMDATA",
        "SYSTEMDRIVE",
        "NUMBER_OF_PROCESSORS",
        # Modelo visual opcional de Liblevo para releer páginas dudosas de escaneos.
        "LIBLEVO_OVIS",
        "LIBLEVO_OVIS_MODEL",
        "LIBLEVO_VISUAL_COMPLETA",
    }
    env = {k: v for k, v in os.environ.items() if k.upper() in allowed}
    models = str(tools / "xberg-modelos")
    env.update(
        HF_HUB_OFFLINE="1",
        TRANSFORMERS_OFFLINE="1",
        HF_HUB_CACHE=models,
        HUGGINGFACE_HUB_CACHE=models,
        XBERG_OCR_MODEL_CACHE_DIR=models,
        XBERG_CACHE_DIR=str(tools / "xberg-cache"),
        OMP_NUM_THREADS=THREADS,
        MKL_NUM_THREADS=THREADS,
        RAYON_NUM_THREADS=THREADS,
        CUDA_VISIBLE_DEVICES="",
        RUST_LOG="off",
        DO_NOT_TRACK="1",
        ORT_DYLIB_PATH=str(tools / "onnxruntime-1.24.4" / "onnxruntime.dll"),
        DOCLING_RS_EP="cpu",
        DOCLING_RS_PDF_WORKERS="1",
        DOCLING_RS_OCR_SESSIONS="1",
        DOCLING_RS_NO_GRAPH_CACHE="1",
        DOCLING_RS_MODELS_DIR=str(tools / "docling-rs-modelos"),
    )
    return env


def command(candidate: str, tools: Path, pdf: Path, output: Path, scanned: bool) -> list[str]:
    if candidate in ("xberg", "xberg-layout", "xberg-capa"):
        config = {
            "pages": {
                "extract_pages": True,
                "insert_page_markers": True,
                "marker_format": "<!-- page: {page_num} -->",
            },
            "ocr": {
                "backend": "paddle-ocr",
                "language": ["en"],
                "auto_rotate": False,
                "vlm_fallback": {"mode": "disabled"},
                "paddle_ocr_settings": {
                    "language": "en",
                    "model_tier": "small",
                    "model_version": "pp-ocrv6",
                    "inference_backend": "ort",
                    "use_angle_cls": False,
                },
            },
        }
        arguments = [
            str(tools / "xberg-1.3.10" / "xberg.exe"),
            "extract",
            "--no-config-discovery",
            "--config-json",
            json.dumps(config),
            "--content-format",
            "markdown",
            "--no-cache",
            "true",
            "--layout",
            *(["--use-layout-for-markdown"] if candidate == "xberg-layout" else []),
            "--layout-table-model",
            "tatr",
            "--acceleration",
            "cpu",
            "--max-threads",
            THREADS,
            "--log-level",
            "error",
        ]
        if scanned and candidate != "xberg-capa":
            arguments += ["--ocr", "true", "--ocr-backend", "paddle-ocr", "--force-ocr", "true"]
        images = output.parent / "images"
        images.mkdir(exist_ok=True)
        arguments += ["--extract-images", "true", "--output-dir", str(images)]
        return [*arguments, str(pdf), "-o", str(output)]
    if candidate == "liblevo":
        # El mismo camino que la app: Xberg en proceso aparte, estilos del PDF y pulido.
        root = Path(__file__).resolve().parents[1]
        return [
            str(root / "target" / "release" / "liblevo-convert.exe"),
            str(root / ".venv" / "Lib" / "site-packages" / "pypdfium2_raw" / "pdfium.dll"),
            str(tools / "xberg-1.3.10"),
            str(tools / "xberg-modelos"),
            str(pdf),
            str(output.parent / "liblevo"),
        ]
    if candidate == "docling-rs":
        arguments = [
            str(tools / "docling-rs-2.22.0" / "docling-rs.exe"),
            "--page-break-placeholder",
            PAGE_BREAK,
            "--output-file",
            str(output),
        ]
        if scanned:
            arguments.append("--force-full-page-ocr")
        return [*arguments, str(pdf)]
    raise ValueError(candidate)


def run_measured(
    arguments: list[str], env: dict[str, str] | None, timeout: float
) -> tuple[int, float, float]:
    started = time.perf_counter()
    child = psutil.Popen(arguments, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
    peak = 0.0
    stop = threading.Event()

    def sample() -> None:
        nonlocal peak
        while not stop.is_set():
            try:
                total = child.memory_info().rss
                for sub in child.children(recursive=True):
                    total += sub.memory_info().rss
                peak = max(peak, total / 2**20)
            except psutil.Error:
                pass
            stop.wait(0.2)

    thread = threading.Thread(target=sample, daemon=True)
    thread.start()
    try:
        _, stderr = child.communicate(timeout=timeout)
        code = child.returncode
    except subprocess.TimeoutExpired:
        child.kill()
        code = -9
    stop.set()
    thread.join()
    return code, time.perf_counter() - started, peak


def renumber(markdown: str, candidate: str, offset: int) -> str:
    if candidate == "docling-rs":
        parts = markdown.split(PAGE_BREAK)
        return "".join(f"<!-- page: {i + 1 + offset} -->\n{part}" for i, part in enumerate(parts))
    return re.sub(
        r"<!-- page: (\d+) -->",
        lambda m: f"\n<!-- page: {int(m.group(1)) + offset} -->\n",
        markdown,
    )


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--candidate",
        choices=["xberg", "xberg-layout", "xberg-capa", "docling-rs", "liblevo"],
        required=True,
    )
    parser.add_argument("--tools", type=Path, required=True)
    parser.add_argument("--manifest", type=Path, default=Path("eval/manifest.json"))
    parser.add_argument("--pdf-dir", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--only", nargs="*", default=[])
    parser.add_argument("--timeout", type=float, default=2 * 3600)
    args = parser.parse_args()

    env = environment(args.tools)
    manifest = json.loads(args.manifest.read_text(encoding="utf-8"))
    for document in manifest["documents"]:
        if args.only and document["id"] not in args.only:
            continue
        target = args.out / document["id"]
        target.mkdir(parents=True, exist_ok=True)
        pdf = args.pdf_dir / f"{document['id']}.pdf"
        print(f"{document['id']}…", flush=True)
        with tempfile.TemporaryDirectory() as temporary:
            folder = Path(temporary)
            offset = 0
            if document.get("pages"):
                first, last = document["pages"]
                pdf = subset(pdf, first, last, folder)
                offset = first - 1
            output = folder / "out.md"
            code, seconds, peak = run_measured(
                command(args.candidate, args.tools, pdf, output, document["kind"] == "scanned"),
                env,
                args.timeout,
            )
            run: dict[str, object] = {"seconds": round(seconds, 2), "peak_mib": round(peak, 1)}
            produced = folder / "liblevo"
            if args.candidate == "liblevo" and (produced / "document.md").exists():
                shutil.copy(produced / "document.md", output)
                report = produced / "run.json"
                if report.exists():
                    produced_run = json.loads(report.read_text(encoding="utf-8"))
                    for key in ("doubtful_pages", "reread_pages", "figure_pages"):
                        if produced_run.get(key) is not None:
                            run[key] = [page + offset for page in produced_run[key]]
                    if "visual_available" in produced_run:
                        run["visual_available"] = produced_run["visual_available"]
                if (produced / "images").is_dir():
                    shutil.copytree(produced / "images", folder / "images", dirs_exist_ok=True)
            if code == 0 and output.exists():
                markdown = renumber(output.read_text(encoding="utf-8"), args.candidate, offset)
                (target / "document.md").write_text(markdown, encoding="utf-8")
                if (folder / "images").is_dir():
                    shutil.copytree(folder / "images", target / "images", dirs_exist_ok=True)
                run["status"] = "ok"
            else:
                run["status"] = "failed"
                run["note"] = f"código de salida {code}"
            (target / "run.json").write_text(json.dumps(run, ensure_ascii=False), encoding="utf-8")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
