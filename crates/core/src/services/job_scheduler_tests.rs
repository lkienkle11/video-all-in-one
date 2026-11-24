use super::job_scheduler_fixtures::*;
use crate::{
    domain::job::{JobKind, JobState},
    ports::JobStore,
};

#[tokio::test]
async fn queued_cancellation_never_calls_runner_and_retry_rejects_nonfailed_states() {
    let mut h = Harness::new();
    let first = h.scheduler.submit(work(JobKind::Cut)).unwrap();
    let run = h.started().await;
    let queued = h.scheduler.submit(work(JobKind::Merge)).unwrap();
    h.scheduler.retry(queued);
    assert!(h.failed(queued).await.contains("Queued"));
    h.scheduler.cancel(queued);
    h.state(queued, JobState::Cancelled).await;
    h.scheduler.retry(queued);
    assert!(h.failed(queued).await.contains("Cancelled"));
    run.finish.send(Outcome::Success).ok().unwrap();
    h.state(first, JobState::Succeeded).await;
    let next = h.scheduler.submit(work(JobKind::Cut)).unwrap();
    let run = h.started().await;
    assert_eq!(run.job.id(), next);
    run.finish.send(Outcome::Success).ok().unwrap();
    h.state(next, JobState::Succeeded).await;
    assert!(h.runner.cleaned.lock().unwrap().is_empty());
}

#[tokio::test]
async fn download_limit_of_two_is_independent_of_encodes() {
    let mut h = Harness::new();
    let a = h.scheduler.submit(work(JobKind::Download)).unwrap();
    let first = h.started().await;
    let b = h.scheduler.submit(work(JobKind::Download)).unwrap();
    let second = h.started().await;
    let c = h.scheduler.submit(work(JobKind::Download)).unwrap();
    let e = h.scheduler.submit(work(JobKind::Cut)).unwrap();
    let encode = h.started().await;
    assert_eq!(encode.job.id(), e);
    assert_eq!(h.store.get(c).unwrap().job.state(), JobState::Queued);
    first.finish.send(Outcome::Success).ok().unwrap();
    h.state(a, JobState::Succeeded).await;
    let third = h.started().await;
    assert_eq!(third.job.id(), c);
    assert_eq!(h.store.get(b).unwrap().job.state(), JobState::Running);
    second.finish.send(Outcome::Success).ok().unwrap();
    third.finish.send(Outcome::Success).ok().unwrap();
    encode.finish.send(Outcome::Success).ok().unwrap();
}

#[tokio::test]
async fn cleanup_failure_leaves_job_failed_without_starting_retry() {
    let mut h = Harness::new();
    let id = h.scheduler.submit(work(JobKind::Cut)).unwrap();
    h.started().await.finish.send(Outcome::Error).ok().unwrap();
    h.state(id, JobState::Failed).await;
    assert!(h.failed(id).await.contains("disk full"));
    *h.runner.cleanup_error.lock().unwrap() = true;
    h.scheduler.retry(id);
    assert!(h.failed(id).await.contains("temp file is locked"));
    assert_eq!(h.store.get(id).unwrap().job.state(), JobState::Failed);
    assert!(h.starts.try_recv().is_err());
}

#[tokio::test]
async fn recover_schedules_persisted_queued_jobs() {
    let mut h = Harness::new();
    let id = h
        .store
        .insert(JobKind::Download, "queued before shutdown")
        .unwrap();
    h.scheduler.recover().await.unwrap();
    let run = h.started().await;
    assert_eq!(run.job.id(), id);
    run.finish.send(Outcome::Success).ok().unwrap();
    h.state(id, JobState::Succeeded).await;
}

#[tokio::test]
async fn recovery_cleans_each_interrupted_job_before_scheduling_queued_work() {
    let mut h = Harness::new();
    let first = h.store.insert(JobKind::Cut, "first export").unwrap();
    let second = h.store.insert(JobKind::Merge, "second export").unwrap();
    for id in [first, second] {
        h.store.transition(id, JobState::Running, None).unwrap();
        h.runner.stale.lock().unwrap().push(id);
    }
    let queued = h.store.insert(JobKind::Cut, "queued export").unwrap();
    h.scheduler.recover().await.unwrap();
    assert!(h.runner.stale.lock().unwrap().is_empty());
    assert_eq!(*h.runner.cleaned.lock().unwrap(), vec![first, second]);
    for id in [first, second] {
        let job = h.store.get(id).unwrap().job;
        assert_eq!(job.state(), JobState::Failed);
        assert_eq!(job.error(), Some("interrupted"));
    }
    let run = h.started().await;
    assert_eq!(run.job.id(), queued);
    run.finish.send(Outcome::Success).ok().unwrap();
    h.state(queued, JobState::Succeeded).await;
}

#[tokio::test]
async fn recovery_cleanup_failure_is_readable_and_does_not_start_queued_work() {
    let h = Harness::new();
    let old = h.store.insert(JobKind::Cut, "interrupted export").unwrap();
    h.store.transition(old, JobState::Running, None).unwrap();
    let queued = h.store.insert(JobKind::Cut, "queued export").unwrap();
    *h.runner.cleanup_error.lock().unwrap() = true;
    let error = h.scheduler.recover().await.unwrap_err();
    assert!(error.to_string().contains("temp file is locked"));
    assert_eq!(h.store.get(old).unwrap().job.state(), JobState::Failed);
    assert_eq!(h.store.get(queued).unwrap().job.state(), JobState::Queued);
}

#[tokio::test]
async fn progress_is_correlated_and_closed_event_receiver_does_not_stop_jobs() {
    use crate::ports::events::JobEvent;
    let mut h = Harness::new();
    let id = h.scheduler.submit(work(JobKind::Cut)).unwrap();
    let run = h.started().await;
    loop {
        if bounded(h.events.recv()).await.unwrap() == JobEvent::Progress(id, 0.5) {
            break;
        }
    }
    h.events.close();
    run.finish.send(Outcome::Success).ok().unwrap();
    let next = h.scheduler.submit(work(JobKind::Cut)).unwrap();
    let run = h.started().await;
    assert_eq!(run.job.id(), next);
    assert_eq!(h.store.get(id).unwrap().job.state(), JobState::Succeeded);
    run.finish.send(Outcome::Success).ok().unwrap();
}

#[tokio::test]
async fn submitted_job_is_persisted_before_runner_starts() {
    let mut h = Harness::new();
    let id = h.scheduler.submit(work(JobKind::Cut)).unwrap();
    assert_eq!(h.store.get(id).unwrap().payload, "same logical export");
    let run = h.started().await;
    assert_eq!(run.job.id(), id);
    assert_eq!(run.persisted.job.state(), JobState::Running);
    assert_eq!(run.job.state(), JobState::Running);
    run.finish.send(Outcome::Success).ok().unwrap();
    h.state(id, JobState::Succeeded).await;
}

#[tokio::test]
async fn encode_limit_of_one_runs_jobs_one_at_a_time() {
    let mut h = Harness::new();
    let first = h.scheduler.submit(work(JobKind::Cut)).unwrap();
    let a = h.started().await;
    let second = h.scheduler.submit(work(JobKind::Merge)).unwrap();
    // A separate download acts as a scheduling barrier while the encoder is held.
    let download = h.scheduler.submit(work(JobKind::Download)).unwrap();
    let d = h.started().await;
    assert_eq!(d.job.id(), download);
    assert_eq!(h.store.get(second).unwrap().job.state(), JobState::Queued);
    assert!(h.starts.try_recv().is_err());
    a.finish.send(Outcome::Success).ok().unwrap();
    let b = h.started().await;
    assert_eq!(b.job.id(), second);
    assert_eq!(h.store.get(first).unwrap().job.state(), JobState::Succeeded);
    b.finish.send(Outcome::Success).ok().unwrap();
    d.finish.send(Outcome::Success).ok().unwrap();
    h.state(second, JobState::Succeeded).await;
}

#[tokio::test]
async fn cancel_running_job_stops_runner_and_marks_cancelled() {
    let mut h = Harness::new();
    let id = h.scheduler.submit(work(JobKind::Cut)).unwrap();
    let run = h.started().await;
    h.scheduler.cancel(id);
    bounded(run.cancelled).await.unwrap();
    assert_eq!(h.store.get(id).unwrap().job.state(), JobState::Running);
    run.finish.send(Outcome::Success).ok().unwrap();
    h.state(id, JobState::Cancelled).await;
}

#[tokio::test]
async fn failed_job_keeps_readable_error() {
    let mut h = Harness::new();
    for outcome in [Outcome::Error, Outcome::Panic] {
        let id = h.scheduler.submit(work(JobKind::Cut)).unwrap();
        h.started().await.finish.send(outcome).ok().unwrap();
        h.state(id, JobState::Failed).await;
        let message = h.failed(id).await;
        assert!(message.contains("disk full") || message.contains("encoder panicked"));
        assert_eq!(h.store.get(id).unwrap().job.error(), Some(message.as_str()));
    }
}

#[tokio::test]
async fn retry_requeues_only_failed_jobs() {
    let mut h = Harness::new();
    let id = h.scheduler.submit(work(JobKind::Cut)).unwrap();
    let run = h.started().await;
    h.scheduler.retry(id);
    assert!(h.failed(id).await.contains("Running"));
    assert!(h.runner.cleaned.lock().unwrap().is_empty());
    run.finish.send(Outcome::Error).ok().unwrap();
    h.state(id, JobState::Failed).await;
    h.scheduler.retry(id);
    let retried = h.started().await;
    assert_eq!(retried.job.id(), id);
    assert_eq!(retried.persisted.payload, "same logical export");
    assert_eq!(retried.job.error(), None);
    assert_eq!(h.store.list().unwrap().len(), 1);
    retried.finish.send(Outcome::Success).ok().unwrap();
    h.state(id, JobState::Succeeded).await;
    h.scheduler.retry(id);
    assert!(h.failed(id).await.contains("Succeeded"));
    assert_eq!(*h.runner.cleaned.lock().unwrap(), vec![id]);
}

#[tokio::test]
async fn retry_deletes_stale_temp_files() {
    let mut h = Harness::new();
    let id = h.scheduler.submit(work(JobKind::Merge)).unwrap();
    h.started().await.finish.send(Outcome::Error).ok().unwrap();
    h.state(id, JobState::Failed).await;
    h.runner.stale.lock().unwrap().push(id);
    h.scheduler.retry(id);
    let retry = h.started().await;
    assert!(h.runner.stale.lock().unwrap().is_empty());
    assert_eq!(*h.runner.cleaned.lock().unwrap(), vec![id]);
    retry.finish.send(Outcome::Success).ok().unwrap();
    h.state(id, JobState::Succeeded).await;
}

#[tokio::test]
async fn recover_marks_interrupted_running_as_failed_and_scheduler_accepts_new_jobs() {
    let mut h = Harness::new();
    let old = h.store.insert(JobKind::Cut, "interrupted export").unwrap();
    h.store.transition(old, JobState::Running, None).unwrap();
    h.scheduler.recover().await.unwrap();
    h.state(old, JobState::Failed).await;
    assert_eq!(h.store.get(old).unwrap().job.error(), Some("interrupted"));
    let new = h.scheduler.submit(work(JobKind::Merge)).unwrap();
    let run = h.started().await;
    assert_eq!(run.job.id(), new);
    run.finish.send(Outcome::Success).ok().unwrap();
    h.state(new, JobState::Succeeded).await;
}

#[tokio::test]
async fn duplicate_submission_of_identical_job_creates_two_independent_jobs() {
    let mut h = Harness::new();
    // Identical requests intentionally remain independent; no silent deduplication.
    let a = h.scheduler.submit(work(JobKind::Download)).unwrap();
    let b = h.scheduler.submit(work(JobKind::Download)).unwrap();
    assert_ne!(a, b);
    let first = h.started().await;
    let second = h.started().await;
    assert_ne!(first.job.id(), second.job.id());
    assert_eq!(first.persisted.payload, second.persisted.payload);
    let failed = first.job.id();
    first.finish.send(Outcome::Error).ok().unwrap();
    h.state(failed, JobState::Failed).await;
    let succeeded = second.job.id();
    second.finish.send(Outcome::Success).ok().unwrap();
    h.state(succeeded, JobState::Succeeded).await;
    assert_eq!(h.store.get(failed).unwrap().job.state(), JobState::Failed);
}
