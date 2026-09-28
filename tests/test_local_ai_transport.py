from __future__ import annotations

import asyncio
import json
from collections.abc import AsyncIterator
from threading import Timer
from time import perf_counter

import httpx
import pytest

import liblevo.local_ai_transport as transport_module
from liblevo.cancellation import CancellationToken
from liblevo.errors import ImprovementError, ProcessingCancelledError
from liblevo.improvement_contracts import MAX_LOCAL_AI_OUTPUT_CHARACTERS
from liblevo.local_ai_client import LocalAiClient
from liblevo.processing_metrics import capture_batch_telemetry


class _PeriodicStream(httpx.AsyncByteStream):
    def __init__(self, chunks: tuple[bytes, ...]) -> None:
        self._chunks = chunks

    async def __aiter__(self) -> AsyncIterator[bytes]:
        for chunk in self._chunks:
            yield chunk


def _transport_with_periodic_chunks(*chunks: bytes) -> httpx.MockTransport:
    return httpx.MockTransport(
        lambda _request: httpx.Response(200, stream=_PeriodicStream(tuple(chunks)))
    )


def test_prediction_limit_scales_with_the_fragment_and_context() -> None:
    assert transport_module.prediction_token_limit(5, 8_192) == 130
    assert transport_module.prediction_token_limit(3_000, 8_192) == 1_128
    assert transport_module.prediction_token_limit(20_000, 8_192) == 2_048
    assert transport_module.prediction_token_limit(20_000, 2_048) == 1_024


def test_active_stream_can_outlive_the_configured_idle_timeout_within_total_deadline() -> None:
    transport = _transport_with_periodic_chunks(
        b'{"message":{"content":"respuesta "}}\n',
        b'{"message":{"content":"completa"}}\n',
        b'{"message":{"content":""},"done":true,"done_reason":"stop"}\n',
    )

    with LocalAiClient(timeout=httpx.Timeout(1), transport=transport) as client:
        result = transport_module.request_local_ai(
            client,
            "liblevo-local",
            8_192,
            "Return the content.",
            "Content",
            None,
        )

    assert result == "respuesta completa"


@pytest.mark.parametrize("context,expected", [(8192, 4096), (2048, 1024)])
def test_reasoning_reserve_is_explicit_and_bounded_by_context(context: int, expected: int) -> None:
    def respond(request: httpx.Request) -> httpx.Response:
        payload = json.loads(request.content)
        assert payload["options"]["num_predict"] == expected
        return httpx.Response(200, json={"response": "complete", "done": True})

    with LocalAiClient(transport=httpx.MockTransport(respond)) as client:
        assert (
            transport_module.request_local_ai_raw(
                client, "review", context, "short", None, minimum_prediction_tokens=4096
            )
            == "complete"
        )


def test_stream_has_a_default_total_deadline_derived_from_the_read_timeout(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    clock = iter((0.0, 601.0))
    monkeypatch.setattr(transport_module, "monotonic", clock.__next__)
    transport = _transport_with_periodic_chunks(
        b'{"message":{"content":"partial"}}\n',
    )

    with LocalAiClient(timeout=httpx.Timeout(120), transport=transport) as client:
        with pytest.raises(ImprovementError, match="máximo total de generación"):
            transport_module.request_local_ai(
                client,
                "liblevo-local",
                8_192,
                "Return the content.",
                "Content",
                None,
            )


def test_stream_fails_only_after_explicit_total_generation_limit(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    clock = iter((0.0, 31.0))
    monkeypatch.setattr(transport_module, "monotonic", clock.__next__)
    transport = _transport_with_periodic_chunks(
        b'{"message":{"content":"partial"}}\n',
    )

    with LocalAiClient(timeout=httpx.Timeout(30), transport=transport) as client:
        with pytest.raises(ImprovementError, match="máximo total de generación"):
            transport_module.request_local_ai(
                client,
                "liblevo-local",
                8_192,
                "Return the content.",
                "Content",
                None,
                max_generation_seconds=30,
            )


def test_stream_reports_only_privacy_safe_local_inference_metrics() -> None:
    transport = _transport_with_periodic_chunks(
        b'{"message":{"content":"respuesta"},"done":false}\n',
        b'{"message":{"content":""},"done":true,"prompt_eval_count":12,'
        b'"eval_count":4,"total_duration":2500000000,"load_duration":500000000,'
        b'"eval_duration":2000000000}\n',
    )
    metrics = []

    with LocalAiClient(timeout=httpx.Timeout(120), transport=transport) as client:
        result = transport_module.request_local_ai(
            client,
            "liblevo-local",
            8_192,
            "Return the content.",
            "Private content that must not enter metrics.",
            None,
            on_metrics=metrics.append,
        )

    assert result == "respuesta"
    assert metrics == [
        transport_module.LocalAiMetrics(
            wall_duration_ms=metrics[0].wall_duration_ms,
            prompt_tokens=12,
            output_tokens=4,
            ollama_total_duration_ms=2_500,
            ollama_load_duration_ms=500,
            output_tokens_per_second=2.0,
        )
    ]


def test_chat_request_keeps_its_existing_payload_contract() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(
            200,
            stream=_PeriodicStream((b'{"message":{"content":"ok"},"done":true}\n',)),
        )

    with LocalAiClient(
        timeout=httpx.Timeout(120), transport=httpx.MockTransport(handler)
    ) as client:
        result = transport_module.request_local_ai(
            client,
            "liblevo-local",
            8_192,
            "Return the content.",
            "Private content",
            None,
            image=b"\x00\xff",
            json_response=True,
        )

    assert result == "ok"
    assert len(requests) == 1
    assert requests[0].url.path == "/api/chat"
    request_json = json.loads(requests[0].read())
    assert request_json["model"] == "liblevo-local"
    assert request_json["messages"] == [
        {"role": "system", "content": "Return the content."},
        {"role": "user", "content": "Private content", "images": ["AP8="]},
    ]
    assert request_json["stream"] is True
    assert request_json["think"] is False
    assert request_json["format"] == "json"
    assert "raw" not in request_json
    assert "keep_alive" not in request_json


def test_raw_generation_uses_the_generate_contract_and_reports_metrics(
    caplog: pytest.LogCaptureFixture,
) -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(
            200,
            stream=_PeriodicStream(
                (
                    b'{"response":"respuesta ","done":false}\n',
                    b'{"response":"raw","done":true,"prompt_eval_count":12,'
                    b'"eval_count":4,"total_duration":2500000000,"load_duration":500000000,'
                    b'"eval_duration":2000000000}\n',
                )
            ),
        )

    metrics = []
    private_prompt = "PRIVATE RAW PROMPT"
    with LocalAiClient(
        timeout=httpx.Timeout(120), transport=httpx.MockTransport(handler)
    ) as client:
        with capture_batch_telemetry() as telemetry:
            result = transport_module.request_local_ai_raw(
                client,
                "specialized-translator:latest",
                8_192,
                private_prompt,
                None,
                on_metrics=metrics.append,
                operation="translation_raw",
                temperature=0.7,
                top_p=0.6,
                top_k=20,
            )

    assert result == "respuesta raw"
    assert len(requests) == 1
    assert requests[0].url.path == "/api/generate"
    request_json = json.loads(requests[0].read())
    assert request_json["model"] == "specialized-translator:latest"
    assert request_json["prompt"] == private_prompt
    assert request_json["stream"] is True
    assert request_json["raw"] is True
    assert request_json["keep_alive"] == 300
    assert request_json["options"]["num_ctx"] == 8_192
    assert request_json["options"]["temperature"] == 0.7
    assert request_json["options"]["top_p"] == 0.6
    assert request_json["options"]["top_k"] == 20
    assert metrics[0].prompt_tokens == 12
    assert metrics[0].output_tokens == 4
    assert telemetry.snapshot().input_characters == len(private_prompt)
    assert private_prompt not in caplog.text


def test_raw_generation_honors_cancellation_between_stream_chunks() -> None:
    cancellation = CancellationToken()

    class _CancellingStream(httpx.AsyncByteStream):
        async def __aiter__(self) -> AsyncIterator[bytes]:
            yield b'{"response":"partial","done":false}\n'
            cancellation.cancel()
            yield b'{"response":"never published","done":true}\n'

    transport = httpx.MockTransport(
        lambda _request: httpx.Response(200, stream=_CancellingStream())
    )
    with LocalAiClient(timeout=httpx.Timeout(120), transport=transport) as client:
        with pytest.raises(ProcessingCancelledError, match="canceló"):
            transport_module.request_local_ai_raw(
                client,
                "specialized-translator:latest",
                8_192,
                "Prompt",
                cancellation,
            )


def test_raw_generation_rejects_an_oversized_streamed_response() -> None:
    oversized = "x" * (MAX_LOCAL_AI_OUTPUT_CHARACTERS + 1)
    transport = _transport_with_periodic_chunks(
        (f'{{"response":{json.dumps(oversized)},"done":false}}\n').encode()
    )
    with LocalAiClient(timeout=httpx.Timeout(120), transport=transport) as client:
        with pytest.raises(ImprovementError, match="tamaño permitido"):
            transport_module.request_local_ai_raw(
                client,
                "specialized-translator:latest",
                8_192,
                "Prompt",
                None,
            )


@pytest.mark.parametrize("raw", [False, True])
@pytest.mark.parametrize("ending", ["missing", "length", "unknown", "error"])
def test_incomplete_generations_never_report_success(
    raw: bool, ending: str, caplog: pytest.LogCaptureFixture
) -> None:
    payload: dict[str, object] = (
        {"response": "PRIVATE partial"} if raw else {"message": {"content": "PRIVATE partial"}}
    )
    if ending == "length":
        payload.update(done=True, done_reason="length")
    elif ending == "unknown":
        payload.update(done=True, done_reason="unexpected")
    elif ending == "error":
        payload = {"error": "PRIVATE server detail"}
    else:
        payload["done"] = False
    transport = _transport_with_periodic_chunks((json.dumps(payload) + "\n").encode())
    metrics: list[transport_module.LocalAiMetrics] = []
    with LocalAiClient(transport=transport) as client:
        with pytest.raises(ImprovementError) as error:
            if raw:
                transport_module.request_local_ai_raw(
                    client, "model", 8_192, "PRIVATE prompt", None, on_metrics=metrics.append
                )
            else:
                transport_module.request_local_ai(
                    client,
                    "model",
                    8_192,
                    "Instructions",
                    "PRIVATE prompt",
                    None,
                    on_metrics=metrics.append,
                )
    assert not metrics
    assert "PRIVATE" not in str(error.value)
    assert "PRIVATE" not in caplog.text
    assert "local_ai_completed" not in caplog.text


@pytest.mark.parametrize("raw", [False, True])
def test_done_is_terminal_and_keeps_content_from_the_final_event(raw: bool) -> None:
    final = {"response": "complete"} if raw else {"message": {"content": "complete"}}
    final.update(done=True, done_reason="stop")

    class TerminalStream(httpx.AsyncByteStream):
        async def __aiter__(self) -> AsyncIterator[bytes]:
            yield (json.dumps(final) + "\n").encode()
            raise AssertionError("The terminal event must close the response stream.")

    transport = httpx.MockTransport(lambda _: httpx.Response(200, stream=TerminalStream()))
    with LocalAiClient(transport=transport) as client:
        if raw:
            result = transport_module.request_local_ai_raw(client, "model", 8_192, "Prompt", None)
        else:
            result = transport_module.request_local_ai(
                client, "model", 8_192, "Instructions", "Prompt", None
            )
    assert result == "complete"


def test_release_model_is_explicit_and_does_not_delete_weights() -> None:
    requests: list[httpx.Request] = []

    def handler(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(200, json={"model": "specialized-translator:latest", "done": True})

    with LocalAiClient(
        timeout=httpx.Timeout(120), transport=httpx.MockTransport(handler)
    ) as client:
        transport_module.release_local_ai_model(client, "specialized-translator:latest")

    assert len(requests) == 1
    assert requests[0].url.path == "/api/generate"
    request_json = json.loads(requests[0].read())
    assert request_json == {
        "model": "specialized-translator:latest",
        "prompt": "",
        "stream": False,
        "keep_alive": 0,
    }


@pytest.mark.parametrize("phase", ["headers", "body", "partial_line"])
@pytest.mark.parametrize("stop", ["cancel", "deadline"])
def test_silent_connections_are_interrupted_and_closed(phase, stop) -> None:
    token = CancellationToken()
    closed = []

    class SilentStream(httpx.AsyncByteStream):
        async def __aiter__(self):
            if phase == "partial_line":
                yield b'{"response":"unfinished'
            await asyncio.Event().wait()

        async def aclose(self):
            closed.append(True)

    async def handler(_request):
        if phase == "headers":
            try:
                await asyncio.Event().wait()
            finally:
                closed.append(True)
        return httpx.Response(200, stream=SilentStream())

    timer = Timer(0.1, token.cancel)
    if stop == "cancel":
        timer.start()
    expected = ProcessingCancelledError if stop == "cancel" else ImprovementError
    try:
        with LocalAiClient(transport=httpx.MockTransport(handler), timeout=120) as client:
            started = perf_counter()
            with pytest.raises(expected):
                transport_module.request_local_ai_raw(
                    client,
                    "local-model",
                    8_192,
                    "Synthetic",
                    token,
                    max_generation_seconds=1 if stop == "deadline" else 120,
                )
            assert perf_counter() - started < (0.8 if stop == "cancel" else 1.8)
            assert closed == [True]
    finally:
        timer.cancel()
        if timer.ident is not None:
            timer.join()


def test_unfinished_ndjson_is_bounded_before_parsing(monkeypatch) -> None:
    monkeypatch.setattr(transport_module, "MAX_LOCAL_AI_OUTPUT_CHARACTERS", 10)

    class OversizedStream(httpx.AsyncByteStream):
        async def __aiter__(self):
            for _ in range(100):
                yield b"x" * 1_024
            pytest.fail("The oversized unfinished line was consumed without a bound")

    with LocalAiClient(
        transport=httpx.MockTransport(lambda _: httpx.Response(200, stream=OversizedStream()))
    ) as client:
        with pytest.raises(ImprovementError, match="tamaño permitido"):
            transport_module.request_local_ai_raw(client, "model", 8_192, "Synthetic", None)


def test_unload_does_not_wait_for_a_silent_server() -> None:
    closed = []

    async def handler(_request):
        try:
            await asyncio.Event().wait()
        finally:
            closed.append(True)

    with LocalAiClient(transport=httpx.MockTransport(handler)) as client:
        started = perf_counter()
        with pytest.raises(ImprovementError, match="a tiempo"):
            transport_module.release_local_ai_model(client, "model")
        assert perf_counter() - started < 1
        assert closed == [True]


@pytest.mark.parametrize("partial_body", [False, True])
def test_cancellation_closes_a_real_loopback_socket(monkeypatch, partial_body) -> None:
    from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
    from threading import Event, Thread

    received, released, disconnected = Event(), Event(), Event()
    token = CancellationToken()

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self):  # noqa: N802
            self.rfile.read(int(self.headers["Content-Length"]))
            if partial_body:
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b'{"response":"')
                self.wfile.flush()
            received.set()
            self.connection.settimeout(3)
            if self.connection.recv(1) == b"":
                disconnected.set()
            released.wait(3)
            self.close_connection = True

        def log_message(self, *args):
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    server.daemon_threads = True
    thread = Thread(target=server.serve_forever, daemon=True)
    thread.start()
    monkeypatch.setattr(
        transport_module, "OLLAMA_BASE_URL", f"http://127.0.0.1:{server.server_port}"
    )

    def cancel_after_request():
        if received.wait(2):
            token.cancel()

    canceller = Thread(target=cancel_after_request, daemon=True)
    canceller.start()
    try:
        with LocalAiClient(timeout=120) as client:
            with pytest.raises(ProcessingCancelledError):
                transport_module.request_local_ai_raw(client, "model", 8_192, "Synthetic", token)
        assert received.is_set()
        assert disconnected.wait(1), "The cancelled HTTP connection remained open"
    finally:
        released.set()
        server.shutdown()
        server.server_close()
        thread.join(2)
        canceller.join(2)
