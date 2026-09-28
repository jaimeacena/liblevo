from pathlib import Path

from liblevo.application.job_queue import JobQueue
from liblevo.application.queue_run_coordinator import QueueRunCoordinator
from liblevo.application.queue_session import QueueSession
from liblevo.application.run_preparation import PreparedQueueRun
from liblevo.application.scheduler import QueueRunPlan, RunMode
from liblevo.domain.jobs import DocumentFormat, DocumentSource, JobConfiguration
from liblevo.settings import AppSettings


def test_queue_run_coordinator_owns_runtime_lookup_claim_and_flags() -> None:
    queue = JobQueue()
    job = queue.add(
        DocumentSource(Path("one.txt"), DocumentFormat.TEXT, 10, 1),
        JobConfiguration(),
        job_id="one",
    )
    session = QueueSession(queue)
    session.ensure_runtime(job.id)
    session.set_prepared_run(PreparedQueueRun(QueueRunPlan(RunMode.NEW, (job.id,)), (), ()))
    coordinator = QueueRunCoordinator(session)

    assert coordinator.validation_issues() == ()
    assert coordinator.pending_items() == ()
    assert coordinator.runtime_for("missing", AppSettings()) is None
    assert coordinator.runtime_for(job.id, AppSettings()) is not None
    assert coordinator.run_flags() == (False, False)
    assert not coordinator.local_ai_required()
    assert coordinator.begin() == (job.id,)
    claimed = coordinator.claim_next(AppSettings())
    assert claimed is not None
    assert claimed.job.id == job.id
    assert claimed.runtime is session.current_runtime
    assert claimed.request.execution_plan is not None
