"""Interrupt and resume a synthetic 20-page PDF with the integrated model."""

from __future__ import annotations

import hashlib
import json
import logging
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

from parsezen.cancellation import CancellationToken  # noqa: E402
from parsezen.direct_models import DIRECT_TRANSLATION_MODEL_ID  # noqa: E402
from parsezen.domain.process_lifecycle import ProcessStage  # noqa: E402
from parsezen.errors import ProcessingCancelledError  # noqa: E402
from parsezen.improvement import ImprovementMode  # noqa: E402
from parsezen.processing import OutputFormat, ProcessRequest, process_document  # noqa: E402
from parsezen.settings import AppSettings  # noqa: E402

SOURCE = PROJECT / "audits" / "2026-09-26-integral-01" / "synthetic-20-pages.pdf"


def _hash(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def _forbid_http(*_args: object, **_kwargs: object) -> object:
    raise AssertionError("La traducción directa intentó HTTP")


class _SafeCounts(logging.Handler):
    def __init__(self) -> None:
        super().__init__()
        self.completed: list[tuple[int, int, int]] = []

    def emit(self, record: logging.LogRecord) -> None:
        match = re.fullmatch(
            r"improvement_completed chunks=(\d+) executed=(\d+) resumed=(\d+) preserved=\d+",
            record.getMessage(),
        )
        if match:
            self.completed.append(tuple(map(int, match.groups())))


def main() -> int:
    destination = ROOT / "resume-trial-2"
    report = ROOT / "resume-trial-2.json"
    if destination.exists() or report.exists():
        raise FileExistsError("No se sobrescribirá un ensayo anterior")
    if not SOURCE.is_file():
        raise FileNotFoundError("Falta el PDF sintético anterior")
    destination.mkdir()
    source_hash = _hash(SOURCE)
    output = destination / "output"
    output.mkdir()
    work = destination / "work"
    epub = destination / "epub"
    request = ProcessRequest(
        SOURCE,
        convert_to_markdown=True,
        output_directory=output,
        output_format=OutputFormat.EPUB,
        improvement_mode=ImprovementMode.TRANSLATE,
        target_language="Español",
        epub_title="Libro sintético de 20 páginas",
        epub_author="Auditoría",
    )
    settings = AppSettings(
        model=DIRECT_TRANSLATION_MODEL_ID,
        context_window=8192,
        timeout_seconds=300.0,
    )
    token = CancellationToken()
    stage: ProcessStage | None = None
    cancelled_at: tuple[int, int] | None = None

    def on_stage(value: ProcessStage) -> None:
        nonlocal stage
        stage = value

    def on_progress(current: int, total: int) -> None:
        nonlocal cancelled_at
        if stage is ProcessStage.TRANSLATING and current >= 5 and cancelled_at is None:
            cancelled_at = (current, total)
            token.cancel()

    metrics = _SafeCounts()
    logger = logging.getLogger("parsezen.improvement")
    previous_level = logger.level
    logger.setLevel(logging.INFO)
    logger.addHandler(metrics)
    try:
        with (
            patch.object(httpx.Client, "request", _forbid_http),
            patch.object(httpx.AsyncClient, "request", _forbid_http),
        ):
            first_started = perf_counter()
            try:
                process_document(
                    request,
                    settings=settings,
                    cancellation=token,
                    on_stage=on_stage,
                    on_progress=on_progress,
                    work_checkpoint_root=work,
                    epub_checkpoint_root=epub,
                )
            except ProcessingCancelledError:
                cancelled = True
            else:
                cancelled = False
            first_seconds = round(perf_counter() - first_started, 3)
            intermediate_files = sum(path.is_file() for path in work.rglob("*"))
            prematurely_published = any(output.glob("*.epub")) if output.exists() else False
            if not cancelled or cancelled_at is None or intermediate_files == 0:
                raise AssertionError("La interrupción no conservó progreso reutilizable")
            if prematurely_published:
                raise AssertionError("Se publicó un EPUB tras cancelar")

            second_started = perf_counter()
            result = process_document(
                request,
                settings=settings,
                work_checkpoint_root=work,
                epub_checkpoint_root=epub,
            )
            second_seconds = round(perf_counter() - second_started, 3)
    finally:
        logger.removeHandler(metrics)
        logger.setLevel(previous_level)

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
        page * 17 for page in range(3, 21) if re.search(rf"\b{page * 17}\b", normalized) is None
    ]
    summary = {
        "cancelled_after_translation_progress": cancelled_at,
        "cancelled_seconds": first_seconds,
        "intermediate_checkpoint_files": intermediate_files,
        "premature_epub": prematurely_published,
        "resume_seconds": second_seconds,
        "improvement_counts": metrics.completed,
        "source_unchanged": _hash(SOURCE) == source_hash,
        "epub_zip_valid": corrupt is None,
        "navigation_present": any(name.endswith("nav.xhtml") for name in names),
        "missing_synthetic_numbers": missing,
        "semantic_quality_human_checked": False,
    }
    report.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(summary, ensure_ascii=False), flush=True)
    return 0 if summary["source_unchanged"] and summary["epub_zip_valid"] and not missing else 1


if __name__ == "__main__":
    raise SystemExit(main())
