"""Exercise the integrated review adapter on a synthetic bilingual sentence."""

from __future__ import annotations

import json
import sys
from pathlib import Path
from time import perf_counter

ROOT = Path(__file__).resolve().parent
PROJECT = ROOT.parents[1]
sys.path.insert(0, str(PROJECT / "src"))

import httpx  # noqa: E402

from parsezen.direct_ai_runtime import DirectAiClient  # noqa: E402
from parsezen.direct_models import DIRECT_REVIEW_MODEL_ID  # noqa: E402
from parsezen.local_ai_adapters import request_adapted_local_ai  # noqa: E402

EVIDENCE = ROOT / "product-direct-review-evidence.json"


def forbid_http(*_args: object, **_kwargs: object) -> object:
    raise AssertionError("Direct review attempted HTTP")


def main() -> int:
    if EVIDENCE.exists():
        raise RuntimeError("Review evidence already exists")
    httpx.AsyncClient.stream = forbid_http
    httpx.Client.request = forbid_http
    started = perf_counter()
    with DirectAiClient(model_root=ROOT) as client:
        response = request_adapted_local_ai(
            client,
            DIRECT_REVIEW_MODEL_ID,
            8192,
            'Revisa la fidelidad. Devuelve solo JSON: {"correcta": boolean, "motivo": string}.',
            'Original: "Ana carried 12 books." Traducción: "Ana llevó 12 libros."',
            None,
            prediction_characters=200,
            max_generation_seconds=120,
            json_response=True,
            operation="translation_review",
        )
    data = json.loads(response)
    checks = {
        "json_object": isinstance(data, dict),
        "correcta_boolean": isinstance(data.get("correcta"), bool),
        "motivo_text": isinstance(data.get("motivo"), str),
    }
    evidence = {
        "case": "PRODUCT-DIRECT-REVIEW-01",
        "result": "PASS" if all(checks.values()) else "FAIL",
        "elapsed_seconds": round(perf_counter() - started, 3),
        "checks": checks,
        "http_forbidden": True,
        "content_logged": False,
        "human_review_equivalence_checked": False,
    }
    EVIDENCE.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(evidence, ensure_ascii=False, indent=2))
    return 0 if evidence["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
