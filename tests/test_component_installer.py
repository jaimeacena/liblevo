from __future__ import annotations

import hashlib
import json
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from threading import Event, Thread
from typing import Any

import httpx
import pytest

import parsezen.component_installer as installer
from parsezen.component_catalog import (
    REVIEW_MODEL_NAME,
    REVIEW_OLLAMA_SOURCE_MODEL,
    TRANSLATION_LICENSE_SHA256,
    TRANSLATION_MODEL_NAME,
    TRANSLATION_OLLAMA_SOURCE_MODEL,
)
from parsezen.errors import LocalModelUnavailableError
from parsezen.local_ai_policy import ComponentCapability, ComponentVerification
from parsezen.local_models import LocalAISetupCancelled, LocalHardware


def _hardware() -> LocalHardware:
    return LocalHardware(
        ram_total_mebibytes=32_768,
        ram_available_mebibytes=16_384,
        disk_free_bytes=50_000_000_000,
    )


@pytest.mark.parametrize(
    ("capability", "source", "alias", "final_path"),
    [
        (
            ComponentCapability.TRANSLATION,
            TRANSLATION_OLLAMA_SOURCE_MODEL,
            TRANSLATION_MODEL_NAME,
            "/api/create",
        ),
        (
            ComponentCapability.REVIEW,
            REVIEW_OLLAMA_SOURCE_MODEL,
            REVIEW_MODEL_NAME,
            "/api/copy",
        ),
    ],
)
def test_installer_uses_only_fixed_source_and_verifies_final_alias(
    monkeypatch: Any,
    capability: ComponentCapability,
    source: str,
    alias: str,
    final_path: str,
) -> None:
    requests: list[tuple[str, dict[str, object]]] = []
    verifications = iter(
        (
            ComponentVerification(False, ("model_not_installed",)),
            ComponentVerification(True),
        )
    )
    monkeypatch.setattr(
        installer, "verify_component_manifest", lambda *_a, **_k: next(verifications)
    )

    def respond(request: httpx.Request) -> httpx.Response:
        payload = json.loads(request.content) if request.content else {}
        requests.append((request.url.path, payload))
        if request.url.path == "/api/pull":
            return httpx.Response(
                200,
                text='{"status":"pulling manifest"}\n{"status":"success"}\n',
            )
        if request.url.path == "/api/create":
            return httpx.Response(200, json={"status": "success"})
        if request.url.path == "/api/copy":
            return httpx.Response(200)
        raise AssertionError(request.url.path)

    result = installer.install_product_component(
        capability,
        transport=httpx.MockTransport(respond),
        hardware=_hardware(),
        local_only_configured=True,
    )

    assert result.valid
    assert [path for path, _payload in requests] == ["/api/pull", final_path]
    assert requests[0][1] == {"model": source, "stream": True}
    final_payload = requests[1][1]
    if capability is ComponentCapability.TRANSLATION:
        assert final_payload["model"] == alias
        assert final_payload["from"] == source
        assert final_payload["stream"] is False
        license_text = final_payload["license"]
        assert isinstance(license_text, str)
        assert hashlib.sha256(license_text.encode()).hexdigest() == TRANSLATION_LICENSE_SHA256
    else:
        assert final_payload == {"source": source, "destination": alias}


def test_installer_fails_closed_before_download_on_identity_mismatch(monkeypatch: Any) -> None:
    monkeypatch.setattr(
        installer,
        "verify_component_manifest",
        lambda *_a, **_k: ComponentVerification(False, ("ollama_digest_mismatch",)),
    )
    requests: list[httpx.Request] = []

    def respond(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(500)

    with pytest.raises(LocalModelUnavailableError, match="requisitos locales"):
        installer.install_product_component(
            ComponentCapability.TRANSLATION,
            transport=httpx.MockTransport(respond),
            hardware=_hardware(),
            local_only_configured=True,
        )

    assert requests == []


def test_installer_rejects_non_product_capability() -> None:
    with pytest.raises(LocalModelUnavailableError, match="no forma parte"):
        installer.install_product_component(
            ComponentCapability.VISUAL,
            hardware=_hardware(),
            local_only_configured=True,
        )


@pytest.mark.parametrize("send_partial_body", [False, True])
def test_cancel_closes_a_silent_local_connection(monkeypatch, send_partial_body: bool) -> None:
    received = Event()
    release = Event()
    expired = Event()
    cancellation = Event()

    class Handler(BaseHTTPRequestHandler):
        def do_POST(self) -> None:  # noqa: N802
            self.rfile.read(int(self.headers["Content-Length"]))
            if send_partial_body:
                self.send_response(200)
                self.end_headers()
                self.wfile.write(b'{"status":')
                self.wfile.flush()
            received.set()
            if not release.wait(4):
                expired.set()
            self.close_connection = True

        def log_message(self, _format: str, *args: object) -> None:
            pass

    server = ThreadingHTTPServer(("127.0.0.1", 0), Handler)
    server.daemon_threads = True
    server_thread = Thread(target=server.serve_forever, daemon=True)
    server_thread.start()
    monkeypatch.setattr(installer, "OLLAMA_BASE_URL", f"http://127.0.0.1:{server.server_port}")

    def cancel_after_request() -> None:
        if received.wait(2):
            cancellation.set()

    cancel_thread = Thread(target=cancel_after_request, daemon=True)
    cancel_thread.start()
    try:
        with pytest.raises(LocalAISetupCancelled):
            installer._pull_fixed_source(
                TRANSLATION_OLLAMA_SOURCE_MODEL,
                on_progress=None,
                cancellation=cancellation,
                transport=None,
            )
        assert received.is_set()
        assert not expired.is_set(), "Cancellation waited for the server to close its connection"
    finally:
        release.set()
        server.shutdown()
        server.server_close()
        server_thread.join(2)
        cancel_thread.join(2)


@pytest.mark.parametrize(
    "response",
    [
        httpx.Response(307, headers={"Location": "https://example.com"}),
        httpx.Response(503),
        httpx.Response(200, content=b"not-json\n"),
        httpx.Response(200, content=b"[]\n"),
        httpx.Response(200, content=b'{"status":"downloading"}\n'),
        httpx.Response(200, content=b'{"error":"private server detail"}\n'),
        httpx.Response(200, content=b"x" * (installer.MAX_PULL_RESPONSE_LINE_BYTES + 1)),
    ],
)
def test_component_pull_rejects_incomplete_or_unsafe_responses(response: httpx.Response) -> None:
    with pytest.raises(LocalModelUnavailableError) as failure:
        installer._pull_fixed_source(
            TRANSLATION_OLLAMA_SOURCE_MODEL,
            on_progress=None,
            cancellation=Event(),
            transport=httpx.MockTransport(lambda _request: response),
        )
    assert "private server detail" not in str(failure.value)


def test_cancelled_component_does_not_start_a_request() -> None:
    cancellation = Event()
    cancellation.set()
    requests: list[httpx.Request] = []

    def respond(request: httpx.Request) -> httpx.Response:
        requests.append(request)
        return httpx.Response(200, text='{"status":"success"}')

    with pytest.raises(LocalAISetupCancelled):
        installer._pull_fixed_source(
            TRANSLATION_OLLAMA_SOURCE_MODEL,
            on_progress=None,
            cancellation=cancellation,
            transport=httpx.MockTransport(respond),
        )
    assert requests == []
