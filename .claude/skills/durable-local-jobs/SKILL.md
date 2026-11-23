---
name: durable-local-jobs
description: Use when implementing or modifying local job scheduling, batching, persistent queues, job recovery, retry, cancellation, SQLite job state, and subprocess lifecycle in Video All-in-One.
---

# Durable Local Jobs

Long-running work (downloads, encoding, export) runs through the Rust application core, never inside UI code.

## Requirements

- SQLite owns the persistent job state (use the `sqlite` skill for WAL, transactions, migrations).
- Persist a job before it is allowed to execute.
- Define explicit states and the valid transitions between them; reject invalid transitions.
- A batch contains independently trackable jobs or steps.
- Apply separate concurrency limits to downloads and to encoding.
- Keep UI state separate from authoritative job state; the UI observes, the core decides.
- On startup, reconcile interrupted jobs (for example `running` with no live process) before scheduling new work.
- Resume only when the operation and its temporary files allow it. Do not claim FFmpeg encoding, or every download, can resume from an exact byte offset.
- Cancellation must stop the associated child process (including children on Windows) and release resources.
- Retry must not duplicate or half-publish output; publish via temp file then atomic rename.
- Cache cleanup must never delete completed outputs.
- Preserve actionable error details (command, exit code, stderr tail) without exposing secrets such as cookies or tokens.
- Subprocess handling must work on Windows, macOS and Linux: binary resolution, path quoting, no shell string interpolation of user input.

## Verification

Test:

- Normal completion
- Download and encoding failures
- Cancellation
- App shutdown during processing
- Crash and restart
- Duplicate job submission
- Missing source files
- Missing or corrupt temporary files
- Low disk space
- Retry and recovery
