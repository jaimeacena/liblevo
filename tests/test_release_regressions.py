"""Regressions reproduced during the September publication audit."""

from __future__ import annotations

import sqlite3
import subprocess
import sys
from io import BytesIO
from pathlib import Path
from time import perf_counter

import pytest
from PIL import Image

from liblevo.application.queue_persistence import QueuePersistenceCoordinator
from liblevo.cover_images import read_cover_image, validate_cover_image
from liblevo.errors import ConversionError
from liblevo.infrastructure.state_store import StateStore
from liblevo.presentation.desktop_instance import DesktopInstance
from liblevo.revision import RevisionKind, build_revision_draft


def test_identical_repeated_document_needs_no_quadratic_alignment(monkeypatch) -> None:
    def unexpected_matcher(*args, **kwargs):
        raise AssertionError("Identical documents need no alignment")

    monkeypatch.setattr("liblevo.revision.SequenceMatcher", unexpected_matcher)
    text = "A repeated paragraph.\n\n" * 4_000
    draft = build_revision_draft(text, text, kinds=frozenset({RevisionKind.CONTENT}))
    assert draft.render() == text
    assert draft.changes == ()


def test_contended_queue_save_returns_promptly_and_retries(tmp_path) -> None:
    store = StateStore(tmp_path / "state.sqlite3")
    coordinator = QueuePersistenceCoordinator(store, interval_seconds=0)
    blocker = sqlite3.connect(store.path)
    try:
        blocker.execute("BEGIN IMMEDIATE")
        started = perf_counter()
        assert not coordinator.persist((), force=True).successful
        assert perf_counter() - started < 0.5
    finally:
        blocker.rollback()
        blocker.close()
    assert coordinator.persist(()).successful


@pytest.mark.parametrize("format", ["PNG", "JPEG", "GIF", "WEBP"])
def test_cover_bytes_are_verified_without_rewriting(format: str, tmp_path) -> None:
    data = BytesIO()
    Image.new("RGB", (3, 2), "white").save(data, format=format)
    payload = data.getvalue()
    path = tmp_path / f"cover.{format.lower()}"
    path.write_bytes(payload)
    assert read_cover_image(path) == payload
    with pytest.raises(ValueError):
        validate_cover_image(path.name, payload[: len(payload) // 2])


def test_cover_rejects_a_disguised_image_and_oversize_before_read(tmp_path, monkeypatch) -> None:
    with pytest.raises(ValueError):
        validate_cover_image("cover.png", b"not an image")
    path = tmp_path / "large.png"
    path.write_bytes(b"123456")
    monkeypatch.setattr("liblevo.cover_images.MAX_COVER_BYTES", 5)
    monkeypatch.setattr(Path, "open", lambda *args, **kwargs: pytest.fail("Unbounded file read"))
    with pytest.raises(ValueError, match="20 MB"):
        read_cover_image(path)


@pytest.mark.parametrize(
    "body",
    [
        "<script>alert(1)</script>",
        '<image href="https://example.invalid/image.png"/>',
        '<rect onclick="alert(1)"/>',
        '<rect style="fill:url(https://example.invalid/image)"/>',
        '<use href="#recursive"/>',
    ],
)
def test_cover_rejects_active_or_remote_svg(body) -> None:
    with pytest.raises(ValueError):
        validate_cover_image(
            "cover.svg", f'<svg xmlns="http://www.w3.org/2000/svg">{body}</svg>'.encode()
        )


def test_cover_accepts_passive_svg() -> None:
    payload = b'<svg xmlns="http://www.w3.org/2000/svg"><rect width="20" height="10"/></svg>'
    assert validate_cover_image("cover.svg", payload) == "image/svg+xml"


def test_epub_physical_bound_precedes_zip_parsing(tmp_path, monkeypatch) -> None:
    from liblevo.epub_conversion import read_editable_epub_package

    path = tmp_path / "oversized.epub"
    path.write_bytes(b"123456")
    monkeypatch.setattr("liblevo.epub_conversion.MAX_EPUB_FILE_BYTES", 5)
    monkeypatch.setattr(
        "liblevo.epub_conversion.is_zipfile", lambda _: pytest.fail("Parsed oversized ZIP")
    )
    with pytest.raises(ConversionError, match="512 MB"):
        read_editable_epub_package(path)


def test_second_instance_activates_owner_without_acquiring_profile(qapp, qtbot, tmp_path) -> None:
    first = DesktopInstance(tmp_path)
    second = DesktopInstance(tmp_path)
    activations = []
    try:
        assert first.acquire()
        first.on_activation(lambda: activations.append(True))
        assert not second.acquire()
        qtbot.waitUntil(lambda: bool(activations))
        other_profile = DesktopInstance(tmp_path / "other")
        try:
            assert other_profile.acquire()
        finally:
            other_profile.close()
    finally:
        second.close()
        first.close()
    replacement = DesktopInstance(tmp_path)
    try:
        assert replacement.acquire()
    finally:
        replacement.close()


def test_profile_lock_is_recovered_after_owner_process_crashes(qapp, tmp_path) -> None:
    code = """
import os, sys
from pathlib import Path
from PySide6.QtCore import QCoreApplication
from liblevo.presentation.desktop_instance import DesktopInstance
app = QCoreApplication([])
instance = DesktopInstance(Path(sys.argv[1]))
assert instance.acquire()
os._exit(0)
"""
    subprocess.run([sys.executable, "-c", code, str(tmp_path)], check=True, timeout=20)
    replacement = DesktopInstance(tmp_path)
    try:
        assert replacement.acquire()
    finally:
        replacement.close()
