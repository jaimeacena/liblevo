"""Two independent synthetic PDF-to-EPUB runs through the integrated model."""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path
from time import perf_counter
from unittest.mock import patch
from xml.etree import ElementTree
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parent
PROJECT = ROOT.parents[1]
sys.path.insert(0, str(PROJECT / "src"))

import httpx  # noqa: E402

from parsezen.direct_models import DIRECT_TRANSLATION_MODEL_ID  # noqa: E402
from parsezen.improvement import ImprovementMode  # noqa: E402
from parsezen.processing import OutputFormat, ProcessRequest, process_document  # noqa: E402
from parsezen.settings import AppSettings  # noqa: E402

SOURCE = PROJECT / "audits" / "2026-09-26-integral-01" / "synthetic-20-pages.pdf"


def _hash(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(8 * 1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _forbid_http(*_args: object, **_kwargs: object) -> object:
    raise AssertionError("Direct PDF translation attempted HTTP")


def main() -> int:
    if not SOURCE.is_file():
        raise FileNotFoundError("The earlier synthetic PDF is absent")
    if (ROOT / "direct-20-runs.json").exists():
        raise FileExistsError("Refusing to overwrite earlier direct-run evidence")
    source_hash = _hash(SOURCE)
    results: list[dict[str, object]] = []
    with (
        patch.object(httpx.Client, "request", _forbid_http),
        patch.object(httpx.AsyncClient, "request", _forbid_http),
    ):
        for run in (1, 2):
            destination = ROOT / f"direct-20-output-{run}"
            if destination.exists():
                raise FileExistsError("Refusing to overwrite an earlier audit output")
            destination.mkdir()
            started = perf_counter()
            result = process_document(
                ProcessRequest(
                    SOURCE,
                    convert_to_markdown=True,
                    output_directory=destination,
                    output_format=OutputFormat.EPUB,
                    improvement_mode=ImprovementMode.TRANSLATE,
                    target_language="Español",
                    epub_title="Libro sintético de 20 páginas",
                    epub_author="Auditoría",
                ),
                settings=AppSettings(
                    model=DIRECT_TRANSLATION_MODEL_ID,
                    context_window=8192,
                    timeout_seconds=300.0,
                ),
                work_checkpoint_root=ROOT / f"direct-20-work-{run}",
                epub_checkpoint_root=ROOT / f"direct-20-epub-{run}",
            )
            elapsed = round(perf_counter() - started, 3)
            with ZipFile(result.final_path) as archive:
                corrupt = archive.testzip()
                names = archive.namelist()
                body = " ".join(
                    " ".join(ElementTree.fromstring(archive.read(name)).itertext())
                    for name in names
                    if name.endswith(".xhtml")
                )
            normalized = re.sub(r"\s+", " ", body)
            missing = [
                page * 17
                for page in range(3, 21)
                if re.search(rf"\b{page * 17}\b", normalized) is None
            ]
            telemetry = result.telemetry
            run_result = {
                "run": run,
                "elapsed_seconds": elapsed,
                "source_unchanged": _hash(SOURCE) == source_hash,
                "epub_zip_valid": corrupt is None,
                "has_navigation": any(name.endswith("nav.xhtml") for name in names),
                "missing_synthetic_numbers": missing,
                "epub_bytes": result.final_path.stat().st_size,
                "chapters_reported": result.epub_chapters,
                "translation_issues_reported": (
                    result.translation_quality_report.total_issues
                    if result.translation_quality_report is not None
                    else None
                ),
                "local_ai_requests": telemetry.batches.local_ai_requests if telemetry else None,
                "local_ai_wall_ms": telemetry.batches.wall_duration_ms if telemetry else None,
                "semantic_quality_human_checked": False,
            }
            results.append(run_result)
            (ROOT / "direct-20-runs.json").write_text(
                json.dumps(results, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
            )
            print(json.dumps(run_result, ensure_ascii=False), flush=True)
    return (
        0
        if all(
            item["source_unchanged"]
            and item["epub_zip_valid"]
            and not item["missing_synthetic_numbers"]
            for item in results
        )
        else 1
    )


if __name__ == "__main__":
    raise SystemExit(main())
