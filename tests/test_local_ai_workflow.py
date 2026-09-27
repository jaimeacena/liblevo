from PySide6.QtWidgets import QMessageBox

from parsezen.presentation.local_ai_controller import LocalAIController
from parsezen.presentation.local_ai_workflow import LocalAIWorkflow
from parsezen.presentation.workspace import ParsezenWorkspace
from parsezen.settings import AppSettings


def test_setup_failure_reaches_the_person_and_restores_workflow_state(qtbot, monkeypatch) -> None:
    workspace = ParsezenWorkspace()
    qtbot.addWidget(workspace)
    controller = LocalAIController(workspace)
    workflow = LocalAIWorkflow(
        controller,
        workspace,
        settings=AppSettings,
        processing_active=lambda: False,
        active_editor=lambda: None,
        parent=workspace,
    )
    messages: list[str] = []
    states: list[bool] = []
    monkeypatch.setattr(
        QMessageBox, "warning", lambda _parent, _title, message: messages.append(message)
    )
    workflow.state_changed.connect(states.append)

    controller.setup_failed.emit("Windows no pudo verificar la firma oficial del instalador.")
    controller.setup_finished.emit()

    assert len(messages) == 1
    assert "verificar la firma" in messages[0]
    assert "volver a intentarlo" in messages[0]
    assert states == [False]
    assert workflow.setup_action is None


def test_stopped_runtime_can_be_started_from_component_page(qtbot, monkeypatch) -> None:
    from parsezen.local_models import OllamaConnection, OllamaStatus
    from parsezen.presentation.local_ai_controller import LocalAIAction

    workspace = ParsezenWorkspace()
    qtbot.addWidget(workspace)
    controller = LocalAIController(workspace)
    monkeypatch.setattr(controller, "discover", lambda _model: False)
    actions = []
    monkeypatch.setattr(controller, "setup", lambda action: actions.append(action) or True)
    workflow = LocalAIWorkflow(
        controller,
        workspace,
        settings=AppSettings,
        processing_active=lambda: False,
        active_editor=lambda: None,
        parent=workspace,
    )
    workflow.model_discovery_succeeded(OllamaConnection(OllamaStatus.STOPPED))
    workflow.show_component_setup()
    setup = workflow.component_setup
    assert setup is not None
    assert setup.runtime_button.text() == "Iniciar IA local"
    assert not setup.runtime_button.isHidden()
    assert all(c.status_label.text() == "Pendiente de comprobar" for c in setup.cards.values())
    setup.runtime_button.click()
    assert actions == [LocalAIAction.START]
    assert not setup.runtime_button.isEnabled()
    assert not setup.refresh_button.isEnabled()
    workflow.ai_setup_progress_changed(None, "Iniciando Ollama…")
    assert setup.runtime_label.text() == "Iniciando Ollama…"
    workflow.ai_setup_finished()
    assert setup.runtime_button.isEnabled()
    workflow.model_discovery_succeeded(OllamaConnection(OllamaStatus.READY))
    assert setup.runtime_button.isHidden()


def test_preparing_ai_preserves_pending_choices_and_updates_open_editor(qtbot, tmp_path):
    from parsezen.component_catalog import TRANSLATION_COMPONENT_MANIFEST as manifest
    from parsezen.domain.jobs import LocalAIComponentSnapshot, LocalAIPolicySnapshot
    from parsezen.presentation.main_window import ParsezenMainWindow

    window = ParsezenMainWindow(auto_discover_ai=False, state_path=tmp_path / "state.sqlite3")
    qtbot.addWidget(window)
    source = tmp_path / "sample.txt"
    source.write_text("Example document.", encoding="utf-8")
    window.add_source_paths([source])
    job = window._job_queue.jobs[0]
    window._configure_job(job.id, None)
    editor = window._active_configuration_dialog
    assert editor is not None
    editor.component_setup_requested.disconnect()
    editor._set_translation_language("es")
    draft = window._job_queue.jobs[0]
    assert draft.configuration.translation.target_language == "es"
    assert not draft.is_configured
    assert not window._state_store.load_jobs()[0].is_configured
    assert window._state_store.load_jobs()[0].configuration.translation.target_language == "es"
    window.set_local_ai_policy_snapshot(
        LocalAIPolicySnapshot(
            translation=LocalAIComponentSnapshot(
                policy_version=manifest.policy_version,
                model=manifest.model_name,
                digest=manifest.ollama_digest,
                context_window=manifest.context_window,
            )
        )
    )
    window._resume_configuration_dialog(editor)
    configured = window._job_queue.jobs[0]
    assert configured.is_configured
    assert configured.configuration.translation.target_language == "es"
    assert configured.configuration.ai.components.translation is not None
    assert editor.validation_label.isHidden()
