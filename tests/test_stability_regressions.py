from __future__ import annotations

import sqlite3
from io import BytesIO
from zipfile import ZipFile

import pytest
from PySide6.QtWidgets import QMessageBox

from parsezen.application.book_editor import BookEditor
from parsezen.application.job_execution import JobExecutionController
from parsezen.application.job_queue import JobQueue
from parsezen.application.review_finalization import ReviewFinalizationCoordinator
from parsezen.application.review_publication import ReviewPublicationCoordinator
from parsezen.application.workspace_recovery import recover_workspace
from parsezen.domain.jobs import (
    DocumentFormat,
    DocumentSource,
    JobConfiguration,
    JobStatus,
    OutputConfiguration,
)
from parsezen.domain.source_identity import SourceIdentity
from parsezen.domain.stages import StageKind
from parsezen.epub_builder import EpubBookMetadata, build_epub, validate_epub_file
from parsezen.infrastructure.artifact_store import ArtifactStore
from parsezen.infrastructure.result_snapshots import ResultSnapshotStore
from parsezen.infrastructure.state_store import StateStore, StateStoreError
from parsezen.pipeline.contracts import ProcessResult
from parsezen.presentation.book_editor_dialog import BookEditorDialog
from parsezen.presentation.epub_confirmation_dialog import EpubConfirmationDialog
from parsezen.presentation.main_window import ParsezenMainWindow
from parsezen.settings import AppSettings


def _setup(tmp_path):
    source = tmp_path / "source.md"
    source.write_text("# Chapter\n\nBody.\n", encoding="utf8")
    destination = tmp_path / "book.epub"
    destination.write_bytes(
        build_epub(source.read_text(), (), EpubBookMetadata("Book", "en")).content
    )
    state = StateStore(tmp_path / "state.sqlite")
    artifacts = ArtifactStore(tmp_path / "artifacts")
    queue = JobQueue()
    job = queue.add(
        DocumentSource.inspect(source),
        JobConfiguration(
            output=OutputConfiguration(configured=True, format=DocumentFormat.EPUB),
        ),
        job_id="job",
    )
    execution = JobExecutionController(queue)
    execution.start_next(job.id)
    execution.advance(job.id, StageKind.PUBLISH)
    execution.block_completed_result_for_review(job.id, StageKind.PUBLISH, review_id="gate")
    state.upsert_job(queue.jobs[0])
    result = ProcessResult(
        destination,
        review_markdown=source.read_text(),
        review_required=True,
        revision_epub_metadata=EpubBookMetadata("Book", "en"),
        preserve_epub_package_on_unchanged_review=True,
    )
    snapshots = ResultSnapshotStore(state, artifacts)
    snapshots.save(
        job.id,
        result,
        source_identity=SourceIdentity(
            job.source.size_bytes,
            job.source.modified_ns,
            job.source.content_sha256,
        ),
    )
    flow = ReviewPublicationCoordinator(
        state, artifacts, ReviewFinalizationCoordinator(queue, execution, state, artifacts)
    )
    return state, artifacts, queue, snapshots, flow, result


@pytest.mark.parametrize(
    "body",
    [
        '<p>Fraction: <math xmlns="http://www.w3.org/1998/Math/MathML"><mfrac><mi>a</mi><mi>b</mi></mfrac></math>.</p>',
        '<table><tr><th scope="col">Price</th></tr><tr><td>125</td></tr></table>',
        '<p>Plain body with <strong>emphasis</strong> and an <a href="#anchor">anchor</a>.</p>'
        '<p id="anchor">Target.</p>',
    ],
)
def test_editor_noop_preserves_original_epub_package(qtbot, tmp_path, body):
    state, artifacts, queue, snapshots, flow, result = _setup(tmp_path)
    original = result.final_path.read_bytes()
    updated = BytesIO()
    with ZipFile(BytesIO(original)) as before, ZipFile(updated, "w") as after:
        for item in before.infolist():
            payload = before.read(item.filename)
            if item.filename.endswith(".xhtml") and b"<p>Body.</p>" in payload:
                payload = payload.replace(b"<p>Body.</p>", body.encode())
            after.writestr(item, payload)
    original = updated.getvalue()
    with ZipFile(BytesIO(original)) as archive:
        assert body.encode() in b"".join(archive.read(name) for name in archive.namelist())
    result.final_path.write_bytes(original)
    book = flow.prepare_book("job", result, result.review_markdown)
    dialog = BookEditorDialog(
        book,
        artifacts,
        job_id="job",
        destination=result.final_path,
        publish_on_accept=False,
        save_draft=lambda draft: state.save_book("job", draft),
    )
    qtbot.addWidget(dialog)
    assert not dialog._has_changes()
    dialog._zoom_in()
    assert not dialog._has_changes()
    dialog.reject()
    assert dialog.saved_for_later
    assert dialog.book == book
    flow.publish_book("job", result, result.review_markdown, dialog.book, ())
    validate_epub_file(result.final_path)
    assert result.final_path.read_bytes() == original
    assert state.load_jobs()[0].status is JobStatus.COMPLETED
    restored = recover_workspace(state.load_jobs(), snapshots, AppSettings())
    assert not restored.reset_paused_job_ids
    assert dict(restored.runtime)["job"].result.final_path == result.final_path


@pytest.mark.parametrize(
    "body",
    [
        "<p>Formula <math><mfrac><mi>a</mi><mi>b</mi></mfrac></math></p>",
        '<table><tr><th scope="col">Header</th></tr></table>',
        '<p class="special">Styled text.</p>',
        "<svg><text>Diagram</text></svg>",
    ],
)
def test_unsupported_content_is_protected_during_metadata_edits(qtbot, tmp_path, monkeypatch, body):
    state, artifacts, queue, snapshots, flow, result = _setup(tmp_path)
    book = flow.prepare_book("job", result, result.review_markdown)
    book = BookEditor(book, artifacts, job_id="job").update_content(book.spine[0], body)
    dialog = BookEditorDialog(
        book, artifacts, job_id="job", destination=None, publish_on_accept=False
    )
    qtbot.addWidget(dialog)
    assert dialog.editor.isReadOnly()
    assert not dialog.split_button.isEnabled()
    assert not dialog.bold_button.isEnabled()
    artifact = dialog.book.section(book.spine[0]).xhtml_artifact_id
    dialog.title_input.setText("Updated title")
    assert dialog._save_current()
    assert dialog.book.metadata.title == "Updated title"
    assert dialog.book.section(book.spine[0]).xhtml_artifact_id == artifact
    dialog._bold()
    dialog._split()
    assert dialog._save_current()
    warnings = []
    monkeypatch.setattr(QMessageBox, "warning", lambda *args: warnings.append(args[1]))
    # A programmatic edit must not bypass the protection either.
    dialog.editor.setPlainText("Flattened content")
    assert not dialog._save_current()
    assert warnings
    assert dialog.book.section(book.spine[0]).xhtml_artifact_id == artifact


def test_zoom_and_metadata_do_not_rewrite_unedited_content(qtbot, tmp_path):
    state, artifacts, queue, snapshots, flow, result = _setup(tmp_path)
    book = flow.prepare_book("job", result, result.review_markdown)
    dialog = BookEditorDialog(book, artifacts, job_id="job", destination=None)
    qtbot.addWidget(dialog)
    dialog._zoom_in()
    dialog.title_input.setText("Updated title")
    assert dialog._save_current()
    assert (
        dialog.book.section(book.spine[0]).xhtml_artifact_id
        == book.section(book.spine[0]).xhtml_artifact_id
    )


@pytest.mark.parametrize("dialog_type", [BookEditorDialog, EpubConfirmationDialog])
def test_save_exit_stays_open_until_sqlite_commits(qtbot, tmp_path, monkeypatch, dialog_type):
    state, artifacts, queue, snapshots, flow, result = _setup(tmp_path)
    book = flow.prepare_book("job", result, result.review_markdown)
    kwargs = (
        {"destination": None, "publish_on_accept": False} if dialog_type is BookEditorDialog else {}
    )
    dialog = dialog_type(
        book,
        artifacts,
        job_id="job",
        save_draft=lambda draft: state.save_book("job", draft),
        **kwargs,
    )
    qtbot.addWidget(dialog)
    dialog.show()
    dialog.title_input.setText("Recover this title")
    if dialog_type is BookEditorDialog:
        dialog.editor.setHtml("<h1>Chapter</h1><p>Recover this paragraph.</p>")
    warnings = []
    monkeypatch.setattr(QMessageBox, "warning", lambda *args: warnings.append(args[1]))
    blocker = sqlite3.connect(state.path)
    blocker.execute("BEGIN IMMEDIATE")
    try:
        dialog.reject()
        assert dialog.isVisible()
        assert not dialog.saved_for_later
        assert warnings == ["No se pudo guardar el borrador"]
    finally:
        blocker.rollback()
        blocker.close()
    dialog.reject()
    assert not dialog.isVisible()
    assert dialog.saved_for_later
    recovered = state.load_book("job")
    assert recovered.metadata.title == "Recover this title"
    if dialog_type is BookEditorDialog:
        assert "Recover this paragraph." in BookEditor(
            recovered, artifacts, job_id="job"
        ).editable_html(recovered.spine[0])


def test_failed_completion_retains_recovery_and_can_retry(tmp_path, monkeypatch):
    state, artifacts, queue, snapshots, flow, result = _setup(tmp_path)
    book = flow.prepare_book("job", result, result.review_markdown)
    original = state.upsert_job

    def fail(_job):
        raise StateStoreError("Busy")

    monkeypatch.setattr(state, "upsert_job", fail)
    with pytest.raises(StateStoreError):
        flow.publish_book("job", result, result.review_markdown, book, ())
    assert queue.jobs[0].status is JobStatus.WAITING_REVIEW
    assert state.load_jobs()[0].status is JobStatus.WAITING_REVIEW
    assert state.load_book("job") == book
    assert snapshots.load("job") is not None
    restored = recover_workspace(state.load_jobs(), snapshots, AppSettings())
    assert not restored.reset_paused_job_ids
    monkeypatch.setattr(state, "upsert_job", original)
    flow.publish_book("job", result, result.review_markdown, book, ())
    assert state.load_jobs()[0].status is JobStatus.COMPLETED
    assert state.load_result_snapshot("job") is None


def test_cleanup_observes_durable_completed_job(tmp_path, monkeypatch):
    state, artifacts, queue, snapshots, flow, result = _setup(tmp_path)
    book = flow.prepare_book("job", result, result.review_markdown)

    def interrupted_cleanup(_job_id):
        recovered = StateStore(state.path).load_jobs()[0]
        assert recovered.status is JobStatus.COMPLETED
        assert recovered.result_path == result.final_path
        raise OSError("Cleanup interrupted")

    monkeypatch.setattr(state, "delete_review_material", interrupted_cleanup)
    published = flow.publish_book("job", result, result.review_markdown, book, ())
    assert published.finalization.warning is not None
    assert state.load_book("job") is not None
    assert state.load_result_snapshot("job") is not None
    restored = recover_workspace(state.load_jobs(), snapshots, AppSettings())
    assert not restored.reset_paused_job_ids
    assert dict(restored.runtime)["job"].result.final_path == result.final_path


def test_close_checks_unsaved_completed_jobs(qtbot, tmp_path, monkeypatch):
    state, artifacts, queue, snapshots, flow, result = _setup(tmp_path)
    window = ParsezenMainWindow(
        settings=AppSettings(),
        state_path=state.path,
        history_path=tmp_path / "history.json",
        work_checkpoint_root=tmp_path / "work",
        auto_discover_ai=False,
    )
    qtbot.addWidget(window)
    window.show()
    window._temporal_timer.stop()
    window._job_execution.complete("job", result.final_path)
    questions = []
    monkeypatch.setattr(
        QMessageBox,
        "question",
        lambda *args: questions.append(args[1]) or QMessageBox.StandardButton.No,
    )
    blocker = sqlite3.connect(state.path)
    blocker.execute("BEGIN IMMEDIATE")
    try:
        assert not window.close()
        assert window.isVisible()
        assert questions == ["Recuperación no disponible"]
    finally:
        blocker.rollback()
        blocker.close()
    assert window.close()
    assert state.load_jobs()[0].status is JobStatus.COMPLETED


@pytest.mark.parametrize("dialog_type", [BookEditorDialog, EpubConfirmationDialog])
def test_publication_keeps_editor_open_if_draft_cannot_be_saved(
    qtbot, tmp_path, monkeypatch, dialog_type
):
    state, artifacts, queue, snapshots, flow, result = _setup(tmp_path)
    book = flow.prepare_book("job", result, result.review_markdown)

    def fail(_draft):
        raise OSError("Disk full")

    kwargs = (
        {"destination": None, "publish_on_accept": False} if dialog_type is BookEditorDialog else {}
    )
    dialog = dialog_type(book, artifacts, job_id="job", save_draft=fail, **kwargs)
    qtbot.addWidget(dialog)
    dialog.show()
    dialog.title_input.setText("Keep my changes")
    warnings = []
    monkeypatch.setattr(QMessageBox, "warning", lambda *args: warnings.append(args[1]))
    dialog._publish()
    assert dialog.isVisible()
    assert dialog.book.metadata.title == "Keep my changes"
    assert warnings == ["No se pudo guardar el borrador"]


def test_closing_main_window_saves_active_editor_before_exit(qtbot, tmp_path, monkeypatch):
    state, artifacts, queue, snapshots, flow, result = _setup(tmp_path)
    book = flow.prepare_book("job", result, result.review_markdown)
    window = ParsezenMainWindow(
        settings=AppSettings(),
        state_path=state.path,
        history_path=tmp_path / "history.json",
        work_checkpoint_root=tmp_path / "work",
        auto_discover_ai=False,
    )
    qtbot.addWidget(window)
    dialog = BookEditorDialog(
        book,
        artifacts,
        job_id="job",
        destination=None,
        save_draft=lambda draft: state.save_book("job", draft),
        parent=window,
    )
    window._active_book_dialog = dialog
    window.show()
    window._temporal_timer.stop()
    dialog.title_input.setText("Saved on application exit")
    monkeypatch.setattr(QMessageBox, "warning", lambda *args: None)
    blocker = sqlite3.connect(state.path)
    blocker.execute("BEGIN IMMEDIATE")
    try:
        assert not window.close()
        assert not dialog.saved_for_later
    finally:
        blocker.rollback()
        blocker.close()
    assert window.close()
    assert state.load_book("job").metadata.title == "Saved on application exit"


def test_renaming_section_retains_unsaved_paragraph_edits(qtbot, tmp_path, monkeypatch):
    from PySide6.QtWidgets import QInputDialog

    state, artifacts, queue, snapshots, flow, result = _setup(tmp_path)
    book = flow.prepare_book("job", result, result.review_markdown)
    dialog = BookEditorDialog(book, artifacts, job_id="job", destination=None)
    qtbot.addWidget(dialog)
    dialog.editor.setHtml("<h1>Chapter</h1><p>Keep this edit before renaming.</p>")
    monkeypatch.setattr(QInputDialog, "getText", lambda *a, **kw: ("Renamed chapter", True))
    dialog._rename()
    assert "Keep this edit before renaming." in dialog._service().editable_html(book.spine[0])
    assert dialog.book.section(book.spine[0]).title == "Renamed chapter"
