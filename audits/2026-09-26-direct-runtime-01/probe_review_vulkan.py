"""Run one synthetic LFM review directly from its GGUF without Ollama."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path
from time import perf_counter

ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / "runtime-vulkan"))

from llama_cpp import Llama  # noqa: E402

MODEL = ROOT / "LFM2.5-2.6B-Q6_K.gguf"
SINGLE_BOS = "--single-bos" in sys.argv
EVIDENCE = ROOT / (
    "direct-review-vulkan-single-bos-evidence.json"
    if SINGLE_BOS
    else "direct-review-vulkan-evidence.json"
)
EXPECTED_SHA256 = "2e74b1a0979a4a1936a408445147d103b8f15b2e2ec31c65fa0166f9069c250d"
PROMPT = (
    "<|startoftext|><|im_start|>system\n"
    "Revisa una frase española de prueba. Responde con un objeto JSON "
    'que tenga exactamente las claves "correcta" (booleano) y "motivo" (texto breve).'
    "<|im_end|>\n"
    "<|im_start|>user\nLa viajera llegó el lunes.<|im_end|>\n"
    "<|im_start|>assistant\n"
)


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(8 * 1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def main() -> int:
    if not MODEL.is_file() or EVIDENCE.exists():
        raise RuntimeError("Verified model missing or evidence already exists")
    model_hash = sha256(MODEL)
    if model_hash != EXPECTED_SHA256:
        raise RuntimeError("GGUF hash differs from the approved upstream artifact")

    started = perf_counter()
    model = Llama(
        model_path=str(MODEL),
        n_gpu_layers=-1,
        n_ctx=8192,
        n_batch=512,
        n_threads=8,
        seed=0,
        use_mmap=True,
        verbose=False,
    )
    loaded_seconds = round(perf_counter() - started, 3)
    result = model.create_completion(
        prompt=PROMPT.removeprefix("<|startoftext|>") if SINGLE_BOS else PROMPT,
        max_tokens=768,
        temperature=0,
        min_p=0,
        seed=0,
        stop=["<|im_end|>", "<|startoftext|>"],
    )
    choice = result["choices"][0]
    answer = choice["text"].strip()
    decoded_json = None
    decoder = json.JSONDecoder()
    for index, character in enumerate(answer):
        if character != "{":
            continue
        try:
            candidate, _ = decoder.raw_decode(answer, index)
        except json.JSONDecodeError:
            continue
        if isinstance(candidate, dict):
            decoded_json = candidate
            break
    checks = {
        "nonempty": bool(answer),
        "finished_without_token_limit": choice["finish_reason"] != "length",
        "valid_review_json": isinstance(decoded_json, dict)
        and isinstance(decoded_json.get("correcta"), bool)
        and isinstance(decoded_json.get("motivo"), str),
    }
    evidence = {
        "case": "DIRECT-REVIEW-VULKAN-01",
        "result": "PASS" if all(checks.values()) else "FAIL",
        "runtime": "llama-cpp-python 0.3.35 Vulkan",
        "model_sha256": model_hash,
        "model_load_seconds": loaded_seconds,
        "total_seconds": round(perf_counter() - started, 3),
        "prompt_tokens": result["usage"]["prompt_tokens"],
        "completion_tokens": result["usage"]["completion_tokens"],
        "finish_reason": choice["finish_reason"],
        "output_sha256": hashlib.sha256(answer.encode("utf-8")).hexdigest(),
        "looks_like_json": "{" in answer and "}" in answer,
        "single_bos_prompt": SINGLE_BOS,
        "checks": checks,
        "output_logged": False,
        "ollama_api_called": False,
    }
    EVIDENCE.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(evidence, ensure_ascii=False, indent=2))
    return 0 if evidence["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
