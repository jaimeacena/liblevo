"""Fixed, locally verified GGUF profiles independent of any model service."""

from __future__ import annotations

import hashlib
import os
from collections.abc import Callable, Mapping
from dataclasses import dataclass
from pathlib import Path
from threading import Event
from types import MappingProxyType
from typing import TYPE_CHECKING
from uuid import uuid4

import httpx
from platformdirs import user_data_path

from parsezen import APP_STORAGE_NAME
from parsezen.cancellation import CancellationToken, check_cancelled
from parsezen.component_catalog import (
    REVIEW_CONTEXT_WINDOW,
    REVIEW_UPSTREAM_FILE,
    REVIEW_UPSTREAM_REPOSITORY,
    REVIEW_UPSTREAM_REVISION,
    REVIEW_UPSTREAM_SHA256,
    TRANSLATION_CONTEXT_WINDOW,
    TRANSLATION_UPSTREAM_FILE,
    TRANSLATION_UPSTREAM_REPOSITORY,
    TRANSLATION_UPSTREAM_REVISION,
    TRANSLATION_UPSTREAM_SHA256,
)
from parsezen.errors import LocalModelUnavailableError
from parsezen.local_ai_policy import ComponentCapability
from parsezen.local_models import LocalAISetupCancelled

if TYPE_CHECKING:
    from parsezen.component_readiness import ComponentReadiness

DIRECT_TRANSLATION_MODEL_ID = "parsezen/hymt-gguf:Q4_K_M"
DIRECT_REVIEW_MODEL_ID = "parsezen/lfm-gguf:Q6_K"


@dataclass(frozen=True, slots=True)
class DirectModelProfile:
    """One reviewed artifact and its model-specific inference contract."""

    capability: ComponentCapability
    model_id: str
    filename: str
    sha256: str
    size_bytes: int
    public_url: str
    context_window: int
    prompt_adapter: str
    stop: tuple[str, ...]
    add_bos_in_binding: bool = False


DIRECT_MODEL_PROFILES: Mapping[str, DirectModelProfile] = MappingProxyType(
    {
        DIRECT_TRANSLATION_MODEL_ID: DirectModelProfile(
            ComponentCapability.TRANSLATION,
            DIRECT_TRANSLATION_MODEL_ID,
            TRANSLATION_UPSTREAM_FILE,
            TRANSLATION_UPSTREAM_SHA256,
            4_624_648_896,
            f"{TRANSLATION_UPSTREAM_REPOSITORY}/resolve/{TRANSLATION_UPSTREAM_REVISION}/"
            f"{TRANSLATION_UPSTREAM_FILE}",
            TRANSLATION_CONTEXT_WINDOW,
            "hymt-translation-v1",
            ("<|startoftext|>", "<|extra_4|>", "<|extra_0|>", "<|eos|>"),
        ),
        DIRECT_REVIEW_MODEL_ID: DirectModelProfile(
            ComponentCapability.REVIEW,
            DIRECT_REVIEW_MODEL_ID,
            REVIEW_UPSTREAM_FILE,
            REVIEW_UPSTREAM_SHA256,
            2_221_615_104,
            f"{REVIEW_UPSTREAM_REPOSITORY}/resolve/{REVIEW_UPSTREAM_REVISION}/"
            f"{REVIEW_UPSTREAM_FILE}",
            REVIEW_CONTEXT_WINDOW,
            "lfm-review-v1",
            ("<|im_end|>", "<|startoftext|>"),
            add_bos_in_binding=True,
        ),
    }
)
ACTIVE_DIRECT_MODEL_IDS: Mapping[ComponentCapability, str] = MappingProxyType(
    {
        ComponentCapability.TRANSLATION: DIRECT_TRANSLATION_MODEL_ID,
        ComponentCapability.REVIEW: DIRECT_REVIEW_MODEL_ID,
    }
)


def direct_model_root() -> Path:
    """Keep downloaded model files separate from documents and application code."""

    return user_data_path(APP_STORAGE_NAME, appauthor=False) / "models"


def direct_profile(model_id: object) -> DirectModelProfile | None:
    return DIRECT_MODEL_PROFILES.get(model_id) if isinstance(model_id, str) else None


def active_direct_profile(capability: ComponentCapability) -> DirectModelProfile | None:
    model_id = ACTIVE_DIRECT_MODEL_IDS.get(capability)
    return DIRECT_MODEL_PROFILES.get(model_id) if model_id is not None else None


def direct_model_present(model_id: str, *, root: Path | None = None) -> bool:
    """Cheap selection hint; full digest verification happens before inference."""

    profile = direct_profile(model_id)
    if profile is None:
        return False
    path = (root if root is not None else direct_model_root()) / profile.filename
    try:
        return path.is_file() and path.stat().st_size == profile.size_bytes
    except OSError:
        return False


def inspect_direct_components(
    *, root: Path | None = None
) -> dict[ComponentCapability, ComponentReadiness]:
    """Check the two fixed capabilities without discovering or contacting Ollama."""

    from importlib.metadata import PackageNotFoundError, version

    from parsezen.component_catalog import PRODUCT_COMPONENT_CATALOG
    from parsezen.component_readiness import ComponentReadiness, ReadinessStatus
    from parsezen.local_models import detect_local_hardware

    try:
        runtime_available = version("llama-cpp-python") == "0.3.35"
        if runtime_available:
            from llama_cpp import Llama  # noqa: F401
    except (ImportError, OSError, PackageNotFoundError):
        runtime_available = False
    hardware = detect_local_hardware()
    results: dict[ComponentCapability, ComponentReadiness] = {}
    for capability in ACTIVE_DIRECT_MODEL_IDS:
        profile = active_direct_profile(capability)
        if profile is None:
            results[capability] = ComponentReadiness(
                capability, ReadinessStatus.INSUFFICIENT, ("manifest_invalid",)
            )
            continue
        requirements = PRODUCT_COMPONENT_CATALOG[profile.capability].requirements
        reasons: list[str] = []
        if (
            requirements.min_ram_mebibytes is not None
            and hardware.ram_available_mebibytes is not None
            and hardware.ram_available_mebibytes < requirements.min_ram_mebibytes
        ):
            reasons.append("ram_insufficient")
        if not runtime_available:
            reasons.append("runtime_unavailable")
        candidate = (root if root is not None else direct_model_root()) / profile.filename
        if candidate.exists():
            try:
                verified_direct_model_path(profile.model_id, root=root)
            except LocalModelUnavailableError:
                reasons.append("direct_digest_mismatch")
            status = ReadinessStatus.PREPARED if not reasons else ReadinessStatus.INSUFFICIENT
        else:
            if (
                requirements.min_disk_free_bytes is not None
                and hardware.disk_free_bytes is not None
                and hardware.disk_free_bytes < requirements.min_disk_free_bytes
            ):
                reasons.append("disk_insufficient")
            status = ReadinessStatus.DOWNLOADABLE if not reasons else ReadinessStatus.INSUFFICIENT
        results[profile.capability] = ComponentReadiness(profile.capability, status, tuple(reasons))
    return results


def verified_direct_model_path(
    model_id: str,
    *,
    root: Path | None = None,
    cancellation: CancellationToken | None = None,
    progress: Callable[[int], None] | None = None,
) -> Path:
    """Verify the fixed file's size and digest before it can process a document."""

    profile = direct_profile(model_id)
    if profile is None:
        raise LocalModelUnavailableError(
            "Este modelo no forma parte del catálogo local verificado."
        )
    path = (root if root is not None else direct_model_root()) / profile.filename
    try:
        if not path.is_file() or path.stat().st_size != profile.size_bytes:
            raise LocalModelUnavailableError("Falta el archivo verificado del modelo local.")
        digest = hashlib.sha256()
        completed = 0
        with path.open("rb") as stream:
            for block in iter(lambda: stream.read(8 * 1024 * 1024), b""):
                check_cancelled(cancellation)
                digest.update(block)
                completed += len(block)
                if progress is not None:
                    progress(min(100, completed * 100 // profile.size_bytes))
        check_cancelled(cancellation)
    except OSError as exc:
        raise LocalModelUnavailableError("No se pudo leer el modelo local.") from exc
    if digest.hexdigest() != profile.sha256:
        raise LocalModelUnavailableError(
            "El archivo del modelo local no coincide con la versión verificada."
        )
    return path


def prepare_direct_model(
    capability: ComponentCapability,
    *,
    root: Path | None = None,
    cancellation: Event | None = None,
    on_progress: Callable[[int | None, str], None] | None = None,
    transport: httpx.BaseTransport | None = None,
) -> Path:
    """Import or download a public GGUF, publishing it only after SHA-256 verification."""

    profile = active_direct_profile(capability)
    if profile is None:
        raise LocalModelUnavailableError("Ese componente no forma parte del catálogo local.")
    destination_root = root if root is not None else direct_model_root()
    destination = destination_root / profile.filename
    if destination.exists():
        return verified_direct_model_path(profile.model_id, root=destination_root)
    destination_root.mkdir(parents=True, exist_ok=True)
    temporary = destination_root / f".{profile.filename}.{uuid4().hex}.part"
    cancel_event = cancellation if cancellation is not None else Event()
    digest = hashlib.sha256()
    total = 0

    def append(block: bytes, output: object) -> None:
        nonlocal total
        if cancel_event.is_set():
            raise LocalAISetupCancelled("Se canceló la preparación del modelo.")
        total += len(block)
        if total > profile.size_bytes:
            raise LocalModelUnavailableError("El archivo del modelo supera el tamaño esperado.")
        digest.update(block)
        output.write(block)  # type: ignore[attr-defined]
        if on_progress is not None:
            on_progress(total * 100 // profile.size_bytes, "Preparando modelo local…")

    try:
        with temporary.open("xb") as output:
            legacy_blob = Path.home() / ".ollama" / "models" / "blobs" / f"sha256-{profile.sha256}"
            if legacy_blob.is_file() and legacy_blob.stat().st_size == profile.size_bytes:
                with legacy_blob.open("rb") as source:
                    for block in iter(lambda: source.read(8 * 1024 * 1024), b""):
                        append(block, output)
            else:
                # This GET fetches a fixed public model binary. Document content is never sent.
                with httpx.Client(
                    timeout=httpx.Timeout(connect=10.0, read=30.0, write=30.0, pool=10.0),
                    follow_redirects=True,
                    max_redirects=5,
                    trust_env=False,
                    transport=transport,
                ) as client:
                    with client.stream("GET", profile.public_url) as response:
                        response.raise_for_status()
                        for block in response.iter_bytes(chunk_size=1024 * 1024):
                            append(block, output)
            output.flush()
            os.fsync(output.fileno())
        if cancel_event.is_set():
            raise LocalAISetupCancelled("Se canceló la preparación del modelo.")
        if total != profile.size_bytes or digest.hexdigest() != profile.sha256:
            raise LocalModelUnavailableError(
                "El modelo descargado no coincide con la versión verificada."
            )
        # Publishing by hard link fails if another process already published a file.
        os.link(temporary, destination)
    except (httpx.HTTPError, OSError) as exc:
        raise LocalModelUnavailableError("No se pudo preparar el modelo local.") from exc
    finally:
        temporary.unlink(missing_ok=True)
    if on_progress is not None:
        on_progress(100, "Modelo local preparado y verificado.")
    return destination
