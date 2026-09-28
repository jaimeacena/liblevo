"""Disable unused Accelerate checkpoint loaders affected by CVE-2026-69112."""

from __future__ import annotations

from types import ModuleType
from typing import NoReturn

from liblevo.errors import ConversionError


def _blocked_checkpoint_loader(*args: object, **kwargs: object) -> NoReturn:
    raise ConversionError(
        "Ese cargador de modelos está desactivado por seguridad. "
        "Liblevo solo admite su pipeline OCR local verificado."
    )


def protect_accelerate_loaders() -> None:
    """Fail closed before importing Docling; its OCR uses independent safe-tensor loaders.

    The pinned dependency still contains the upstream defect. This narrow mitigation
    removes the two unused entry points and all their public aliases from our process;
    it does not claim to patch Accelerate for use outside Liblevo.
    """
    import accelerate
    from accelerate import big_modeling, utils
    from accelerate.utils import modeling

    targets: tuple[tuple[ModuleType, str], ...] = (
        (accelerate, "load_checkpoint_in_model"),
        (utils, "load_checkpoint_in_model"),
        (modeling, "load_checkpoint_in_model"),
        (big_modeling, "load_checkpoint_in_model"),
        (accelerate, "load_checkpoint_and_dispatch"),
        (big_modeling, "load_checkpoint_and_dispatch"),
    )
    for module, name in targets:
        if not callable(getattr(module, name, None)):
            raise RuntimeError("Cambió el contrato del cargador OCR; revisa su protección.")
        setattr(module, name, _blocked_checkpoint_loader)
