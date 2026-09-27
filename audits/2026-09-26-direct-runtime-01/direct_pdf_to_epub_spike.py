"""Synthetic end-to-end PDF translation with direct GGUF and blocked HTTP calls.

This is an isolated adapter experiment. It changes no application source files.
"""

from __future__ import annotations

import hashlib
import json
import re
import sys
from pathlib import Path
from time import monotonic, perf_counter
from xml.etree import ElementTree
from zipfile import ZipFile

ROOT = Path(__file__).resolve().parent
REPOSITORY = ROOT.parents[1]
sys.path.insert(0, str(ROOT / "runtime-vulkan"))
sys.path.insert(0, str(REPOSITORY / "src"))

import httpx  # noqa: E402
from llama_cpp import Llama  # noqa: E402

import parsezen.improvement as improvement  # noqa: E402
import parsezen.local_ai_adapters as adapters  # noqa: E402
import parsezen.processing as processing  # noqa: E402
from parsezen.cancellation import check_cancelled  # noqa: E402
from parsezen.component_catalog import (  # noqa: E402
    TRANSLATION_CONTEXT_WINDOW,
    TRANSLATION_MODEL_NAME,
)
from parsezen.errors import ImprovementError  # noqa: E402
from parsezen.improvement import ImprovementMode  # noqa: E402
from parsezen.improvement_contracts import MAX_LOCAL_AI_OUTPUT_CHARACTERS  # noqa: E402
from parsezen.local_ai_transport import prediction_token_limit  # noqa: E402
from parsezen.processing import OutputFormat, ProcessRequest, process_document  # noqa: E402
from parsezen.settings import AppSettings  # noqa: E402

SOURCE = REPOSITORY / "audits" / "2026-09-26-integral-01" / "synthetic-translation-2-pages.pdf"
OUTPUT = ROOT / "direct-epub-output"
EVIDENCE = ROOT / "direct-epub-evidence.json"
MODEL = ROOT / "Hy-MT2-7B-Q4_K_M.gguf"
STOP = ["<|startoftext|>", "<|extra_4|>", "<|extra_0|>", "<|eos|>"]


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def block_http(*_args: object, **_kwargs: object) -> object:
    raise AssertionError("The direct-runtime spike attempted HTTP")


def main() -> int:
    if not SOURCE.is_file() or not MODEL.is_file() or OUTPUT.exists() or EVIDENCE.exists():
        raise RuntimeError("Synthetic input or verified model absent, or output already exists")
    original_digest = digest(SOURCE)
    model = Llama(
        model_path=str(MODEL),
        n_gpu_layers=-1,
        n_ctx=TRANSLATION_CONTEXT_WINDOW,
        n_batch=512,
        n_threads=8,
        seed=0,
        use_mmap=True,
        verbose=False,
    )
    calls = 0
    generated_tokens = 0

    def direct_raw(
        _client: object,
        selected_model: str,
        context_window: int,
        prompt: str,
        cancellation: object,
        *,
        prediction_characters: int | None = None,
        max_generation_seconds: float | None = None,
        operation: str = "raw",
        on_metrics: object = None,
        temperature: float = 0.0,
        top_p: float | None = None,
        top_k: int | None = None,
        minimum_prediction_tokens: int = 0,
    ) -> str:
        nonlocal calls, generated_tokens
        del operation, on_metrics
        if selected_model != TRANSLATION_MODEL_NAME:
            raise ImprovementError("The spike supports only the fixed translation model")
        check_cancelled(cancellation)
        limit = prediction_token_limit(
            prediction_characters if prediction_characters is not None else len(prompt),
            context_window,
        )
        limit = max(limit, minimum_prediction_tokens)
        deadline = monotonic() + (max_generation_seconds or 600.0)
        stream = model.create_completion(
            prompt=prompt,
            max_tokens=limit,
            temperature=temperature,
            top_p=top_p if top_p is not None else 0.95,
            top_k=top_k if top_k is not None else 40,
            min_p=0,
            repeat_penalty=1.1,
            seed=0,
            stop=STOP,
            stream=True,
        )
        parts: list[str] = []
        length = 0
        finish_reason = None
        for item in stream:
            check_cancelled(cancellation)
            if monotonic() > deadline:
                raise ImprovementError("The direct model exceeded its generation deadline")
            choice = item["choices"][0]
            content = choice["text"]
            length += len(content)
            if length > MAX_LOCAL_AI_OUTPUT_CHARACTERS:
                raise ImprovementError("The direct model exceeded the output limit")
            parts.append(content)
            if choice["finish_reason"] is not None:
                finish_reason = choice["finish_reason"]
        if finish_reason != "stop":
            raise ImprovementError("The direct model did not confirm a complete response")
        calls += 1
        generated_tokens += limit
        return "".join(parts)

    adapters.request_local_ai_raw = direct_raw
    adapters.release_local_ai_model = lambda *_args: None
    improvement.is_ollama_local_only_configured = lambda: True
    processing.build_local_visual_text_arbiter = lambda *_args, **_kwargs: None
    httpx.AsyncClient.stream = block_http
    httpx.Client.request = block_http

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
            epub_title="Prueba directa de traducción",
            epub_author="Auditoría",
        ),
        settings=AppSettings(
            model=TRANSLATION_MODEL_NAME,
            context_window=TRANSLATION_CONTEXT_WINDOW,
            timeout_seconds=300.0,
        ),
        work_checkpoint_root=ROOT / "direct-work-checkpoints",
        epub_checkpoint_root=ROOT / "direct-epub-checkpoints",
    )
    with ZipFile(result.final_path) as archive:
        corrupt_member = archive.testzip()
        names = archive.namelist()
        content = " ".join(
            " ".join(ElementTree.fromstring(archive.read(name)).itertext())
            for name in names
            if name.endswith(".xhtml")
        )
    normalized = re.sub(r"\s+", " ", content)
    literals = ("12", "7:30", "48", "84", "Ana", "Bruno")
    missing = [
        item for item in literals if re.search(rf"\b{re.escape(item)}\b", normalized) is None
    ]
    checks = {
        "original_unchanged": digest(SOURCE) == original_digest,
        "zip_integrity": corrupt_member is None,
        "has_navigation": any(name.endswith("nav.xhtml") for name in names),
        "numbers_and_names_present": not missing,
        "direct_model_called": calls > 0,
    }
    evidence = {
        "case": "DIRECT-PDF-EPUB-02",
        "result": "PASS" if all(checks.values()) else "FAIL",
        "elapsed_seconds": round(perf_counter() - started, 3),
        "direct_generation_calls": calls,
        "maximum_allowed_tokens_sum": generated_tokens,
        "output_bytes": result.final_path.stat().st_size,
        "reported_chapters": result.epub_chapters,
        "reported_translation_issues": (
            result.translation_quality_report.total_issues
            if result.translation_quality_report is not None
            else None
        ),
        "checks": checks,
        "http_forbidden_in_process": True,
        "output_logged": False,
        "semantic_fidelity_human_reviewed": False,
    }
    EVIDENCE.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(evidence, ensure_ascii=False, indent=2))
    return 0 if evidence["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
