"""Run the real Parsezen pipeline through its integrated direct GGUF branch."""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path
from time import perf_counter
from xml.etree import ElementTree
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parent
PROJECT = ROOT.parents[1]
sys.path.insert(0, str(ROOT / "runtime-vulkan"))
sys.path.insert(0, str(PROJECT / "src"))

import httpx  # noqa: E402

import parsezen.direct_models as direct_models  # noqa: E402
from parsezen.direct_models import DIRECT_TRANSLATION_MODEL_ID  # noqa: E402
from parsezen.improvement import ImprovementMode  # noqa: E402
from parsezen.processing import OutputFormat, ProcessRequest, process_document  # noqa: E402
from parsezen.settings import AppSettings  # noqa: E402

SOURCE = PROJECT / "audits" / "2026-09-26-integral-01" / "synthetic-translation-2-pages.pdf"
OUTPUT = ROOT / "product-direct-v3-output"
EVIDENCE = ROOT / "product-direct-v3-evidence.json"


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(8 * 1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def forbid_http(*_args: object, **_kwargs: object) -> object:
    raise AssertionError("The integrated direct path attempted HTTP")


def main() -> int:
    if not SOURCE.is_file() or OUTPUT.exists() or EVIDENCE.exists():
        raise RuntimeError("The synthetic source is missing or evidence already exists")
    original_sha = sha256(SOURCE)
    direct_models.direct_model_root = lambda: ROOT
    httpx.AsyncClient.stream = forbid_http
    httpx.Client.request = forbid_http
    OUTPUT.mkdir()
    started = perf_counter()
    result = process_document(
        ProcessRequest(
            SOURCE,
            convert_to_markdown=True,
            output_directory=OUTPUT,
            output_format=OutputFormat.EPUB,
            improvement_mode=ImprovementMode.TRANSLATE,
            target_language="Español",
            epub_title="Prueba integrada sin Ollama",
            epub_author="Auditoría",
        ),
        settings=AppSettings(model=DIRECT_TRANSLATION_MODEL_ID, context_window=8192),
        work_checkpoint_root=ROOT / "product-direct-v3-work-checkpoints",
        epub_checkpoint_root=ROOT / "product-direct-v3-epub-checkpoints",
    )
    with ZipFile(result.final_path) as archive:
        invalid = archive.testzip()
        names = archive.namelist()
        body = " ".join(
            " ".join(ElementTree.fromstring(archive.read(name)).itertext())
            for name in names
            if name.endswith(".xhtml")
        )
    normalized = re.sub(r"\s+", " ", body)
    checks = {
        "original_unchanged": sha256(SOURCE) == original_sha,
        "epub_zip_integrity": invalid is None,
        "epub_navigation": any(name.endswith("nav.xhtml") for name in names),
        "facts_preserved": all(
            re.search(rf"\b{re.escape(value)}\b", normalized)
            for value in ("12", "7:30", "48", "84", "Ana", "Bruno")
        ),
    }
    evidence = {
        "case": "PRODUCT-DIRECT-03",
        "result": "PASS" if all(checks.values()) else "FAIL",
        "elapsed_seconds": round(perf_counter() - started, 3),
        "output_bytes": result.final_path.stat().st_size,
        "checks": checks,
        "http_forbidden": True,
        "model_sha256": direct_models.direct_profile(DIRECT_TRANSLATION_MODEL_ID).sha256,
        "content_logged": False,
        "human_quality_checked": False,
    }
    EVIDENCE.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(evidence, ensure_ascii=False, indent=2))
    return 0 if evidence["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
