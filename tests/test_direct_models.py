from __future__ import annotations

import hashlib
from pathlib import Path
from threading import Event
from types import MappingProxyType

import httpx
import pytest
from PySide6.QtCore import QRunnable

import parsezen.direct_models as direct_models
import parsezen.presentation.local_ai_controller as controller_module
from parsezen.component_readiness import ComponentReadiness, ReadinessStatus
from parsezen.direct_models import (
    DIRECT_TRANSLATION_MODEL_ID,
    DirectModelProfile,
    active_direct_profile,
    direct_profile,
    prepare_direct_model,
    verified_direct_model_path,
)
from parsezen.errors import LocalModelUnavailableError
from parsezen.local_ai_policy import ComponentCapability
from parsezen.local_models import LocalAISetupCancelled, OllamaStatus
from parsezen.presentation.local_ai_controller import LocalAIController


def _tiny_profile(content: bytes, *, digest: str | None = None) -> DirectModelProfile:
    return DirectModelProfile(
        ComponentCapability.TRANSLATION,
        DIRECT_TRANSLATION_MODEL_ID,
        "tiny.gguf",
        digest or hashlib.sha256(content).hexdigest(),
        len(content),
        "https://huggingface.co/example/resolve/fixed/tiny.gguf",
        8192,
        "hymt-translation-v1",
        ("<end>",),
    )


def test_direct_model_import_publishes_only_the_verified_complete_file(
    monkeypatch, tmp_path
) -> None:
    content = b"synthetic public model bytes"
    profile = _tiny_profile(content)
    monkeypatch.setattr(
        direct_models,
        "DIRECT_MODEL_PROFILES",
        MappingProxyType({profile.model_id: profile}),
    )
    requests: list[str] = []

    def serve(request: httpx.Request) -> httpx.Response:
        requests.append(str(request.url))
        return httpx.Response(200, content=content)

    output = prepare_direct_model(
        ComponentCapability.TRANSLATION,
        root=tmp_path,
        transport=httpx.MockTransport(serve),
    )

    assert requests == [profile.public_url]
    assert output.read_bytes() == content
    assert verified_direct_model_path(profile.model_id, root=tmp_path) == output
    assert not list(tmp_path.glob("*.part"))


@pytest.mark.parametrize("cancel", [False, True])
def test_direct_model_import_never_publishes_bad_or_cancelled_content(
    monkeypatch, tmp_path, cancel
) -> None:
    content = b"synthetic public model bytes"
    profile = _tiny_profile(content, digest="0" * 64)
    monkeypatch.setattr(
        direct_models,
        "DIRECT_MODEL_PROFILES",
        MappingProxyType({profile.model_id: profile}),
    )
    event = Event()
    if cancel:
        event.set()
    with pytest.raises(LocalAISetupCancelled if cancel else LocalModelUnavailableError):
        prepare_direct_model(
            ComponentCapability.TRANSLATION,
            root=tmp_path,
            cancellation=event,
            transport=httpx.MockTransport(lambda _request: httpx.Response(200, content=content)),
        )
    assert not (tmp_path / profile.filename).exists()
    assert not list(tmp_path.glob("*.part"))


def test_direct_model_digest_mismatch_blocks_inference(monkeypatch, tmp_path) -> None:
    content = b"synthetic model bytes"
    profile = _tiny_profile(content, digest="0" * 64)
    monkeypatch.setattr(
        direct_models,
        "DIRECT_MODEL_PROFILES",
        MappingProxyType({profile.model_id: profile}),
    )
    (tmp_path / profile.filename).write_bytes(content)

    with pytest.raises(LocalModelUnavailableError, match="no coincide"):
        verified_direct_model_path(profile.model_id, root=tmp_path)


def test_new_active_model_does_not_replace_an_older_job_identity(monkeypatch, tmp_path) -> None:
    old = _tiny_profile(b"older synthetic model")
    current_bytes = b"new synthetic model"
    current = DirectModelProfile(
        ComponentCapability.TRANSLATION,
        "parsezen/future-translation:test",
        "future.gguf",
        hashlib.sha256(current_bytes).hexdigest(),
        len(current_bytes),
        "https://huggingface.co/example/resolve/fixed/future.gguf",
        8192,
        "hymt-translation-v1",
        ("<end>",),
    )
    monkeypatch.setattr(
        direct_models,
        "DIRECT_MODEL_PROFILES",
        MappingProxyType({old.model_id: old, current.model_id: current}),
    )
    monkeypatch.setattr(
        direct_models,
        "ACTIVE_DIRECT_MODEL_IDS",
        MappingProxyType({ComponentCapability.TRANSLATION: current.model_id}),
    )

    output = prepare_direct_model(
        ComponentCapability.TRANSLATION,
        root=tmp_path,
        transport=httpx.MockTransport(lambda _request: httpx.Response(200, content=current_bytes)),
    )

    assert active_direct_profile(ComponentCapability.TRANSLATION) == current
    assert direct_profile(old.model_id) == old
    assert output.name == current.filename


class _ImmediatePool:
    def start(self, worker: QRunnable) -> None:
        worker.run()


def test_direct_ui_discovers_prepared_model_without_contacting_ollama(monkeypatch) -> None:
    readiness = {
        ComponentCapability.TRANSLATION: ComponentReadiness(
            ComponentCapability.TRANSLATION, ReadinessStatus.PREPARED
        ),
        ComponentCapability.REVIEW: ComponentReadiness(
            ComponentCapability.REVIEW, ReadinessStatus.DOWNLOADABLE
        ),
    }
    monkeypatch.setattr(controller_module, "inspect_direct_components", lambda: readiness)
    monkeypatch.setattr(
        controller_module,
        "discover_ollama",
        lambda _model: pytest.fail("Ollama was contacted in direct mode"),
    )
    controller = LocalAIController(thread_pool=_ImmediatePool(), direct=True)
    connections: list[object] = []
    controller.discovery_succeeded.connect(connections.append)

    assert controller.discover(DIRECT_TRANSLATION_MODEL_ID)
    assert len(connections) == 1
    assert connections[0].status is OllamaStatus.READY
    assert [model.model_id for model in connections[0].models] == [DIRECT_TRANSLATION_MODEL_ID]


def test_direct_ui_installs_by_fixed_capability(monkeypatch, tmp_path: Path) -> None:
    calls: list[ComponentCapability] = []

    def prepare(capability: ComponentCapability, **_kwargs: object) -> Path:
        calls.append(capability)
        return tmp_path / "tiny.gguf"

    monkeypatch.setattr(controller_module, "prepare_direct_model", prepare)
    monkeypatch.setattr(
        controller_module,
        "install_product_component",
        lambda *_args, **_kwargs: pytest.fail("Ollama installer was used"),
    )
    controller = LocalAIController(thread_pool=_ImmediatePool(), direct=True)
    assert controller.install_component(ComponentCapability.TRANSLATION)
    assert calls == [ComponentCapability.TRANSLATION]
