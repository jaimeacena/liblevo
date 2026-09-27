"""Open the real desktop component view with isolated state and a direct model."""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
PROJECT = ROOT.parents[1]
sys.path.insert(0, str(PROJECT / "src"))

from PySide6.QtCore import QEventLoop, QTimer  # noqa: E402
from PySide6.QtWidgets import QApplication  # noqa: E402

from parsezen.component_catalog import TRANSLATION_UPSTREAM_SHA256  # noqa: E402
from parsezen.direct_models import DIRECT_TRANSLATION_MODEL_ID  # noqa: E402
from parsezen.local_ai_policy import ComponentCapability  # noqa: E402
from parsezen.presentation.main_window import ParsezenMainWindow  # noqa: E402
from parsezen.settings import AppSettings  # noqa: E402


def main() -> int:
    evidence_path = ROOT / "direct-ui-evidence.json"
    screenshot_path = ROOT / "direct-ui-components.png"
    if evidence_path.exists() or screenshot_path.exists():
        raise RuntimeError("UI evidence already exists")
    application = QApplication.instance() or QApplication([])
    window = ParsezenMainWindow(
        settings=AppSettings(),
        auto_discover_ai=False,
        history_path=ROOT / "direct-ui-history.json",
        work_checkpoint_root=ROOT / "direct-ui-checkpoints",
        state_path=ROOT / "direct-ui-state.sqlite3",
    )
    window.resize(1100, 720)
    window.show()
    workflow = window._local_ai_workflow  # noqa: SLF001
    loop = QEventLoop()
    workflow._controller.discovery_finished.connect(loop.quit)  # noqa: SLF001
    QTimer.singleShot(30_000, loop.quit)
    workflow.show_component_setup()
    if workflow.discovering:
        loop.exec()
    application.processEvents()
    readiness = workflow.component_readiness
    translation = readiness.get(ComponentCapability.TRANSLATION)
    snapshot = window._local_ai_policy.translation  # noqa: SLF001
    screenshot_saved = window.grab().save(str(screenshot_path))
    checks = {
        "translation_prepared": translation is not None and translation.prepared,
        "direct_identity": snapshot is not None
        and snapshot.digest == TRANSLATION_UPSTREAM_SHA256
        and snapshot.model == DIRECT_TRANSLATION_MODEL_ID,
        "screenshot_saved": screenshot_saved,
        "discovery_finished": not workflow.discovering,
    }
    evidence = {
        "case": "DIRECT-UI-01",
        "result": "PASS" if all(checks.values()) else "FAIL",
        "checks": checks,
        "human_visual_acceptance": False,
        "personal_documents_loaded": False,
    }
    evidence_path.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding="utf-8")
    window.close()
    print(json.dumps(evidence, ensure_ascii=False, indent=2))
    return 0 if evidence["result"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
