from __future__ import annotations

import sys
from pathlib import Path
from types import SimpleNamespace

import pytest

import liblevo.direct_ai_runtime as direct_runtime
from liblevo.cancellation import CancellationToken
from liblevo.direct_models import DIRECT_TRANSLATION_MODEL_ID
from liblevo.errors import ImprovementError, ProcessingCancelledError


def test_direct_runtime_accepts_only_a_complete_bounded_response(monkeypatch, tmp_path) -> None:
    instances = []
    generation_options = []

    class FakeLlama:
        def __init__(self, **_kwargs: object) -> None:
            self.closed = False
            instances.append(self)

        def create_completion(self, **kwargs: object):
            generation_options.append(kwargs)
            yield {"choices": [{"text": "Doce", "finish_reason": None}]}
            yield {"choices": [{"text": " libros", "finish_reason": "stop"}]}

        def tokenize(self, content: bytes, *, add_bos: bool) -> list[int]:
            assert not add_bos
            return list(content)

        def close(self) -> None:
            self.closed = True

    monkeypatch.setitem(sys.modules, "llama_cpp", SimpleNamespace(Llama=FakeLlama))
    monkeypatch.setattr(
        direct_runtime,
        "verified_direct_model_path",
        lambda *_args, **_kwargs: tmp_path / "verified.gguf",
    )
    with direct_runtime.DirectAiClient(model_root=tmp_path) as client:
        response = client.generate_raw(DIRECT_TRANSLATION_MODEL_ID, 8192, "Source", None)
    assert response == "Doce libros"
    assert instances[0].closed
    assert generation_options[0]["repeat_penalty"] == 1.0


@pytest.mark.parametrize("finish_reason", [None, "length"])
def test_direct_runtime_rejects_incomplete_generation(monkeypatch, tmp_path, finish_reason) -> None:
    class FakeLlama:
        def __init__(self, **_kwargs: object) -> None:
            pass

        def create_completion(self, **_kwargs: object):
            yield {"choices": [{"text": "Texto parcial", "finish_reason": finish_reason}]}

        def close(self) -> None:
            pass

    monkeypatch.setitem(sys.modules, "llama_cpp", SimpleNamespace(Llama=FakeLlama))
    monkeypatch.setattr(
        direct_runtime,
        "verified_direct_model_path",
        lambda *_args, **_kwargs: tmp_path / "verified.gguf",
    )
    with direct_runtime.DirectAiClient(model_root=tmp_path) as client:
        with pytest.raises(ImprovementError, match="respuesta completa"):
            client.generate_raw(DIRECT_TRANSLATION_MODEL_ID, 8192, "Source", None)


def test_direct_runtime_honors_cancellation_between_tokens(monkeypatch, tmp_path: Path) -> None:
    token = CancellationToken()

    class FakeLlama:
        def __init__(self, **_kwargs: object) -> None:
            pass

        def create_completion(self, **_kwargs: object):
            yield {"choices": [{"text": "Primer", "finish_reason": None}]}
            token.cancel()
            yield {"choices": [{"text": " segundo", "finish_reason": "stop"}]}

        def close(self) -> None:
            pass

    monkeypatch.setitem(sys.modules, "llama_cpp", SimpleNamespace(Llama=FakeLlama))
    monkeypatch.setattr(
        direct_runtime,
        "verified_direct_model_path",
        lambda *_args, **_kwargs: tmp_path / "verified.gguf",
    )
    with direct_runtime.DirectAiClient(model_root=tmp_path) as client:
        with pytest.raises(ProcessingCancelledError):
            client.generate_raw(DIRECT_TRANSLATION_MODEL_ID, 8192, "Source", token)
