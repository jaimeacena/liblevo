"""Bounded in-process GGUF inference for the fixed local model profiles."""

from __future__ import annotations

import logging
import os
from collections.abc import Callable, Iterator
from pathlib import Path
from time import monotonic
from typing import Any, Self

from liblevo.cancellation import CancellationToken, check_cancelled
from liblevo.direct_models import DirectModelProfile, direct_profile, verified_direct_model_path
from liblevo.errors import ImprovementError, LocalModelUnavailableError
from liblevo.improvement_contracts import MAX_LOCAL_AI_OUTPUT_CHARACTERS
from liblevo.local_ai_transport import LocalAiMetrics, prediction_token_limit
from liblevo.processing_metrics import record_local_ai_request

LOGGER = logging.getLogger(__name__)


class DirectAiClient:
    """Own one native model per processing phase; never opens a network socket."""

    def __init__(self, *, model_root: Path | None = None) -> None:
        self._root = model_root
        self._model: Any = None
        self._profile: DirectModelProfile | None = None
        self._load_duration_ms = 0

    def __enter__(self) -> Self:
        return self

    def __exit__(self, *_exc: object) -> None:
        self.close()

    def close(self) -> None:
        model = self._model
        self._model = None
        self._profile = None
        if model is not None:
            model.close()

    def _loaded_model(
        self, model_id: str, cancellation: CancellationToken | None
    ) -> tuple[Any, DirectModelProfile]:
        profile = direct_profile(model_id)
        if profile is None:
            raise LocalModelUnavailableError(
                "Este modelo no forma parte del catálogo local verificado."
            )
        if self._profile == profile and self._model is not None:
            return self._model, profile
        self.close()
        path = verified_direct_model_path(model_id, root=self._root, cancellation=cancellation)
        check_cancelled(cancellation)
        try:
            from llama_cpp import Llama
        except (ImportError, OSError) as exc:
            raise LocalModelUnavailableError(
                "Falta el motor local de modelos. Repara la instalación de Liblevo."
            ) from exc
        started = monotonic()
        try:
            model = Llama(
                model_path=str(path),
                n_gpu_layers=-1,
                n_ctx=profile.context_window,
                n_batch=512,
                n_threads=min(8, os.cpu_count() or 4),
                seed=0,
                use_mmap=True,
                verbose=False,
            )
        except (RuntimeError, ValueError, OSError) as exc:
            raise LocalModelUnavailableError(
                "El motor local no pudo cargar el modelo verificado en este equipo."
            ) from exc
        self._load_duration_ms = max(0, round((monotonic() - started) * 1_000))
        self._model = model
        self._profile = profile
        return model, profile

    def generate_raw(
        self,
        model_id: str,
        context_window: int,
        prompt: str,
        cancellation: CancellationToken | None,
        *,
        prediction_characters: int | None = None,
        max_generation_seconds: float | None = None,
        operation: str = "raw",
        on_metrics: Callable[[LocalAiMetrics], None] | None = None,
        temperature: float = 0.0,
        top_p: float | None = None,
        top_k: int | None = None,
        minimum_prediction_tokens: int = 0,
    ) -> str:
        """Generate one complete bounded answer using only a verified GGUF file."""

        check_cancelled(cancellation)
        model, profile = self._loaded_model(model_id, cancellation)
        check_cancelled(cancellation)
        if context_window > profile.context_window:
            raise ImprovementError("El contexto solicitado supera el del modelo verificado.")
        if profile.add_bos_in_binding:
            prompt = prompt.removeprefix("<|startoftext|>")
        limit = prediction_token_limit(
            prediction_characters if prediction_characters is not None else len(prompt),
            context_window,
        )
        limit = max(limit, min(minimum_prediction_tokens, max(context_window // 2, 128)))
        seconds = min(max(max_generation_seconds or 600.0, 1.0), 1_800.0)
        deadline = monotonic() + seconds
        started = monotonic()
        chunks: list[str] = []
        length = 0
        finish_reason: str | None = None
        try:
            stream: Iterator[dict[str, Any]] = model.create_completion(
                prompt=prompt,
                max_tokens=limit,
                temperature=temperature,
                top_p=0.95 if top_p is None else top_p,
                top_k=40 if top_k is None else top_k,
                min_p=0,
                repeat_penalty=1.0,
                seed=0,
                stop=list(profile.stop),
                stream=True,
            )
            for item in stream:
                check_cancelled(cancellation)
                if monotonic() > deadline:
                    raise ImprovementError("El modelo local superó el tiempo máximo de generación.")
                choice = item["choices"][0]
                fragment = choice["text"]
                if not isinstance(fragment, str):
                    raise ImprovementError("El modelo local devolvió una respuesta incompatible.")
                length += len(fragment)
                if length > MAX_LOCAL_AI_OUTPUT_CHARACTERS:
                    raise ImprovementError("La respuesta del modelo supera el tamaño permitido.")
                chunks.append(fragment)
                if choice["finish_reason"] is not None:
                    finish_reason = choice["finish_reason"]
        except (KeyError, IndexError, TypeError, ValueError, RuntimeError) as exc:
            raise ImprovementError("El motor local no pudo completar la respuesta.") from exc
        check_cancelled(cancellation)
        if finish_reason != "stop":
            raise ImprovementError("El modelo local no confirmó una respuesta completa.")
        response = "".join(chunks)
        if not response.strip():
            raise ImprovementError("El modelo local devolvió una respuesta vacía.")
        output_tokens = len(model.tokenize(response.encode("utf-8"), add_bos=False))
        wall_ms = max(0, round((monotonic() - started) * 1_000))
        metrics = LocalAiMetrics(
            wall_duration_ms=wall_ms,
            prompt_tokens=len(model.tokenize(prompt.encode("utf-8"), add_bos=False)),
            output_tokens=output_tokens,
            ollama_total_duration_ms=0,
            ollama_load_duration_ms=self._load_duration_ms,
            output_tokens_per_second=round(output_tokens * 1_000 / wall_ms, 2) if wall_ms else 0.0,
        )
        LOGGER.info(
            "direct_ai_completed operation=%s wall_ms=%d prompt_tokens=%d output_tokens=%d",
            operation,
            wall_ms,
            metrics.prompt_tokens,
            output_tokens,
        )
        if on_metrics is not None:
            on_metrics(metrics)
        record_local_ai_request(
            input_characters=len(prompt),
            prompt_tokens=metrics.prompt_tokens,
            output_tokens=metrics.output_tokens,
            wall_duration_ms=wall_ms,
            ollama_total_duration_ms=0,
            ollama_load_duration_ms=self._load_duration_ms,
            operation=operation,
        )
        return response
