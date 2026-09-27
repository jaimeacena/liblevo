"""Bounded access to Ollama's fixed, local native API."""

from __future__ import annotations

import asyncio
import json
import logging
from base64 import b64encode
from collections.abc import AsyncIterator, Callable
from dataclasses import dataclass
from time import monotonic

import httpx

from parsezen.cancellation import CancellationToken, check_cancelled
from parsezen.errors import ImprovementError
from parsezen.improvement_contracts import MAX_LOCAL_AI_OUTPUT_CHARACTERS
from parsezen.local_ai_client import LocalAiClient
from parsezen.local_models import OLLAMA_BASE_URL
from parsezen.processing_metrics import record_local_ai_request

LOGGER = logging.getLogger(__name__)
_DEFAULT_MAX_GENERATION_SECONDS = 600.0
_MIN_MAX_GENERATION_SECONDS = 60.0
_MAX_MAX_GENERATION_SECONDS = 1_800.0
_RAW_GENERATION_KEEP_ALIVE_SECONDS = 300


@dataclass(frozen=True, slots=True)
class LocalAiMetrics:
    """Privacy-safe timing and token counters reported by one local inference."""

    wall_duration_ms: int
    prompt_tokens: int
    output_tokens: int
    ollama_total_duration_ms: int
    ollama_load_duration_ms: int
    output_tokens_per_second: float


def request_local_ai(
    client: LocalAiClient,
    model: str,
    context_window: int,
    instructions: str,
    document_fragment: str,
    cancellation: CancellationToken | None,
    *,
    prediction_characters: int | None = None,
    max_generation_seconds: float | None = None,
    image: bytes | None = None,
    json_response: bool = False,
    operation: str = "other",
    on_metrics: Callable[[LocalAiMetrics], None] | None = None,
) -> str:
    """Return one deterministic local transformation with independent stream bounds."""

    user_message: dict[str, object] = {"role": "user", "content": document_fragment}
    if image is not None:
        user_message["images"] = [b64encode(image).decode("ascii")]
    request_payload = {
        "model": model,
        "messages": [
            {"role": "system", "content": instructions},
            user_message,
        ],
        "stream": True,
        "think": False,
        "options": {
            "temperature": 0,
            "num_ctx": context_window,
            "num_predict": prediction_token_limit(
                prediction_characters
                if prediction_characters is not None
                else len(document_fragment),
                context_window,
            ),
            "seed": 0,
        },
    }
    if json_response:
        request_payload["format"] = "json"
    return _stream_local_ai_response(
        client,
        f"{OLLAMA_BASE_URL}/api/chat",
        request_payload,
        cancellation,
        max_generation_seconds,
        input_characters=len(document_fragment),
        operation=operation,
        raw_response=False,
        on_metrics=on_metrics,
    )


def request_local_ai_raw(
    client: LocalAiClient,
    model: str,
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
    """Generate one complete prompt with Ollama's raw ``/api/generate`` contract.

    ``prompt`` is sent unchanged and therefore owns the complete prompt format.  The
    model is kept loaded between fragment requests; callers release it explicitly
    with :func:`release_local_ai_model` after the phase completes.
    """

    if isinstance(temperature, bool) or not isinstance(temperature, (int, float)):
        raise ValueError("La temperatura local no es válida.")
    if not 0.0 <= float(temperature) <= 2.0:
        raise ValueError("La temperatura local no es válida.")
    if top_p is not None and (
        isinstance(top_p, bool) or not isinstance(top_p, (int, float)) or not 0.0 < top_p <= 1.0
    ):
        raise ValueError("El muestreo top-p local no es válido.")
    if top_k is not None and (
        isinstance(top_k, bool) or not isinstance(top_k, int) or not 1 <= top_k <= 1_000
    ):
        raise ValueError("El muestreo top-k local no es válido.")
    if (
        isinstance(minimum_prediction_tokens, bool)
        or not isinstance(minimum_prediction_tokens, int)
        or not 0 <= minimum_prediction_tokens <= 4_096
    ):
        raise ValueError("La reserva de generación local no es válida.")
    token_limit = prediction_token_limit(
        prediction_characters if prediction_characters is not None else len(prompt),
        context_window,
    )
    token_limit = max(token_limit, min(minimum_prediction_tokens, max(context_window // 2, 128)))
    generation_options: dict[str, int | float] = {
        "temperature": float(temperature),
        "num_ctx": context_window,
        "num_predict": token_limit,
        "seed": 0,
    }
    if top_p is not None:
        generation_options["top_p"] = float(top_p)
    if top_k is not None:
        generation_options["top_k"] = top_k

    request_payload = {
        "model": model,
        "prompt": prompt,
        "stream": True,
        "raw": True,
        "think": False,
        "keep_alive": _RAW_GENERATION_KEEP_ALIVE_SECONDS,
        "options": generation_options,
    }
    return _stream_local_ai_response(
        client,
        f"{OLLAMA_BASE_URL}/api/generate",
        request_payload,
        cancellation,
        max_generation_seconds,
        input_characters=len(prompt),
        operation=operation,
        raw_response=True,
        on_metrics=on_metrics,
    )


def _stream_local_ai_response(
    client: LocalAiClient,
    url: str,
    request_payload: dict[str, object],
    cancellation: CancellationToken | None,
    max_generation_seconds: float | None,
    *,
    input_characters: int,
    operation: str,
    raw_response: bool,
    on_metrics: Callable[[LocalAiMetrics], None] | None,
) -> str:
    check_cancelled(cancellation)

    async def guarded() -> str:
        generation = asyncio.create_task(
            _consume_local_ai_response(
                client,
                url,
                request_payload,
                cancellation,
                max_generation_seconds,
                input_characters=input_characters,
                operation=operation,
                raw_response=raw_response,
                on_metrics=on_metrics,
            )
        )
        try:
            async with asyncio.timeout(_generation_timeout_seconds(client, max_generation_seconds)):
                while not generation.done():
                    check_cancelled(cancellation)
                    await asyncio.wait({generation}, timeout=0.05)
                check_cancelled(cancellation)
                return await generation
        except TimeoutError as exc:
            raise ImprovementError(
                "El modelo local superó el tiempo máximo total de generación."
            ) from exc
        finally:
            generation.cancel()
            await asyncio.gather(generation, return_exceptions=True)

    return client.run(guarded())


async def _bounded_response_lines(response: httpx.Response) -> AsyncIterator[str]:
    # Includes JSON escaping and metadata; bounds incomplete lines and blank-line floods.
    line_limit = MAX_LOCAL_AI_OUTPUT_CHARACTERS * 6 + 65_536
    total_limit = line_limit * 2
    pending = bytearray()
    total = 0
    async for chunk in response.aiter_bytes():
        total += len(chunk)
        if total > total_limit:
            raise ImprovementError("La respuesta del modelo supera el tamaño permitido.")
        fragments = chunk.split(b"\n")
        for index, fragment in enumerate(fragments):
            if len(pending) + len(fragment) > line_limit:
                raise ImprovementError("La respuesta del modelo supera el tamaño permitido.")
            pending.extend(fragment)
            if index < len(fragments) - 1:
                try:
                    yield pending.decode("utf-8")
                except UnicodeError as exc:
                    raise ImprovementError("Ollama devolvió una respuesta incompatible.") from exc
                pending.clear()
    if pending:
        try:
            yield pending.decode("utf-8")
        except UnicodeError as exc:
            raise ImprovementError("Ollama devolvió una respuesta incompatible.") from exc


async def _consume_local_ai_response(
    client: LocalAiClient,
    url: str,
    request_payload: dict[str, object],
    cancellation: CancellationToken | None,
    max_generation_seconds: float | None,
    *,
    input_characters: int,
    operation: str,
    raw_response: bool,
    on_metrics: Callable[[LocalAiMetrics], None] | None,
) -> str:
    """Share bounded NDJSON streaming while keeping endpoint payloads concrete."""

    check_cancelled(cancellation)
    started_at = monotonic()
    generation_timeout = _generation_timeout_seconds(client, max_generation_seconds)
    deadline = started_at + generation_timeout
    prompt_tokens = 0
    output_tokens = 0
    ollama_total_duration_ns = 0
    ollama_load_duration_ns = 0
    ollama_eval_duration_ns = 0
    async with client.http.stream("POST", url, json=request_payload) as response:
        if response.is_redirect:
            raise ImprovementError("Ollama intentó redirigir la solicitud y fue bloqueado.")
        try:
            response.raise_for_status()
        except httpx.HTTPStatusError as exc:
            raise ImprovementError(
                f"El modelo local respondió con el estado HTTP {response.status_code}."
            ) from exc

        content_parts: list[str] = []
        content_length = 0
        saw_done = False
        async for line in _bounded_response_lines(response):
            check_cancelled(cancellation)
            if monotonic() > deadline:
                raise ImprovementError(
                    "El modelo local superó el tiempo máximo total de generación."
                )
            if not line.strip():
                continue
            try:
                payload = json.loads(line)
                if not isinstance(payload, dict):
                    raise TypeError
                if payload.get("error"):
                    raise ImprovementError("Ollama no pudo completar la generación local.")
                if raw_response:
                    content = payload["response"]
                else:
                    message = payload["message"]
                    content = message["content"]
            except ImprovementError:
                raise
            except (json.JSONDecodeError, KeyError, TypeError) as exc:
                raise ImprovementError("Ollama devolvió una respuesta incompatible.") from exc
            if not isinstance(content, str):
                raise ImprovementError("Ollama devolvió una respuesta incompatible.")
            if payload.get("done") is True:
                reason = payload.get("done_reason")
                if reason == "length":
                    raise ImprovementError(
                        "El modelo local agotó el límite de generación; "
                        "la respuesta está incompleta."
                    )
                if reason not in (None, "stop"):
                    raise ImprovementError("Ollama no confirmó una finalización normal.")
                saw_done = True
                prompt_tokens = _safe_nonnegative_int(payload.get("prompt_eval_count"))
                output_tokens = _safe_nonnegative_int(payload.get("eval_count"))
                ollama_total_duration_ns = _safe_nonnegative_int(payload.get("total_duration"))
                ollama_load_duration_ns = _safe_nonnegative_int(payload.get("load_duration"))
                ollama_eval_duration_ns = _safe_nonnegative_int(payload.get("eval_duration"))
            content_length += len(content)
            if content_length > MAX_LOCAL_AI_OUTPUT_CHARACTERS:
                raise ImprovementError("La respuesta del modelo supera el tamaño permitido.")
            content_parts.append(content)
            if saw_done:
                break
        check_cancelled(cancellation)
    if not saw_done:
        raise ImprovementError("Ollama interrumpió la respuesta sin confirmar su finalización.")
    wall_duration_ms = max(0, round((monotonic() - started_at) * 1_000))
    tokens_per_second = (
        output_tokens / (ollama_eval_duration_ns / 1_000_000_000)
        if output_tokens and ollama_eval_duration_ns
        else 0.0
    )
    metrics = LocalAiMetrics(
        wall_duration_ms=wall_duration_ms,
        prompt_tokens=prompt_tokens,
        output_tokens=output_tokens,
        ollama_total_duration_ms=ollama_total_duration_ns // 1_000_000,
        ollama_load_duration_ms=ollama_load_duration_ns // 1_000_000,
        output_tokens_per_second=round(tokens_per_second, 2),
    )
    LOGGER.info(
        "local_ai_completed operation=%s wall_ms=%d prompt_tokens=%d output_tokens=%d "
        "ollama_total_ms=%d load_ms=%d output_tokens_per_second=%.2f",
        operation,
        metrics.wall_duration_ms,
        metrics.prompt_tokens,
        metrics.output_tokens,
        metrics.ollama_total_duration_ms,
        metrics.ollama_load_duration_ms,
        metrics.output_tokens_per_second,
    )
    if on_metrics is not None:
        on_metrics(metrics)
    record_local_ai_request(
        input_characters=input_characters,
        prompt_tokens=metrics.prompt_tokens,
        output_tokens=metrics.output_tokens,
        wall_duration_ms=metrics.wall_duration_ms,
        ollama_total_duration_ms=metrics.ollama_total_duration_ms,
        ollama_load_duration_ms=metrics.ollama_load_duration_ms,
        operation=operation,
    )
    return "".join(content_parts)


def release_local_ai_model(client: LocalAiClient, model: str) -> None:
    """Best-effort bounded unload; a silent Ollama must not delay cancellation."""

    async def release() -> None:
        async with asyncio.timeout(0.5):
            async with client.http.stream(
                "POST",
                f"{OLLAMA_BASE_URL}/api/generate",
                json={"model": model, "prompt": "", "stream": False, "keep_alive": 0},
            ) as response:
                if response.is_redirect:
                    raise ImprovementError("Ollama intentó redirigir la solicitud y fue bloqueado.")
                try:
                    response.raise_for_status()
                except httpx.HTTPStatusError as exc:
                    raise ImprovementError("Ollama no pudo liberar el modelo local.") from exc

    try:
        client.run(release())
    except TimeoutError as exc:
        raise ImprovementError("Ollama no confirmó la liberación del modelo a tiempo.") from exc


def _generation_timeout_seconds(
    client: LocalAiClient,
    requested: float | None,
) -> float:
    if requested is not None and requested > 0:
        return min(max(float(requested), 1.0), _MAX_MAX_GENERATION_SECONDS)
    read_timeout = client.timeout.read
    if isinstance(read_timeout, (int, float)) and read_timeout > 0:
        return min(
            max(float(read_timeout) * 5, _MIN_MAX_GENERATION_SECONDS),
            _MAX_MAX_GENERATION_SECONDS,
        )
    return _DEFAULT_MAX_GENERATION_SECONDS


def _safe_nonnegative_int(value: object) -> int:
    return value if isinstance(value, int) and not isinstance(value, bool) and value >= 0 else 0


def prediction_token_limit(source_characters: int, context_window: int) -> int:
    """Bound runaway generations while leaving ample room for Latin-script output."""

    proportional_limit = (source_characters + 2) // 3 + 128
    context_limit = min(max(context_window // 2, 128), 2_048)
    return max(128, min(proportional_limit, context_limit))
