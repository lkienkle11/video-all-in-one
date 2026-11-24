# Updater installation evidence — Task 7

Date: 2026-11-24. Host: macOS, restricted worker sandbox. This is implementation
and unit-test evidence, **not proof of an installed release updating itself**.
Task 12 must supply that proof with real production installers.

## Transport and verification boundary

`HttpUpdateBackend` implements the synchronous core port with reqwest blocking
and rustls. `new(Url, PathBuf)` returns `Result<Self, UpdateError>`: this deliberately
refines the plan's infallible signature so an insecure URL is refused at
construction without a panic. No existing caller needed changing.

Production accepts HTTPS only; a constructor compiled only for unit tests permits
HTTP on localhost or 127.0.0.1. Redirects cannot downgrade HTTPS, leave the allowed
transport policy, carry credentials, or exceed ten hops; tests prove a redirect target
outside the policy is never contacted (a second server records hits, with a control
case that is followed). Connection timeout is ten seconds. The manifest client keeps a
120-second total deadline. Downloads have **no total limit**: reqwest's blocking client
has no read timeout, so a helper thread reads the body and the caller waits at most 30
seconds for each next chunk (and for the response headers); a stalled transfer fails,
removes the partial file and abandons the thread (TCP keepalive ends it when the
socket dies). Manifests are capped at 1 MiB and must be UTF-8. Downloads stream through
a 64 KiB buffer to an exclusive new file under the canonical work directory, created
with mode 0700 on unix. Names are generated from hashed asset metadata and local
uniqueness, never URL path components. File size is checked while streaming (also
for chunked bodies without Content-Length) and at EOF, SHA-256 is computed incrementally,
and any error or panic in the progress callback removes the partial file through a
drop guard; cleanup errors never replace the original error. The core service
performs signature verification before installation; the adapter rechecks staged
size and digest immediately before using the file.

`install()` never exits the process. After a successful install that needs a restart
the backend calls an injected `RestartHook` (`with_restart_hook`, default: nothing);
the application decides how to quit. `with_data_dir` tells the helper where to write
`update-error.txt` (default: the work folder; the application should pass the folder
that holds `settings.json`). Not yet wired: nothing in the application constructs the
backend, so `main.rs` has no hook yet; the later task that does must pass a hook that
quits the app (a plain `std::process::exit(0)` in `main.rs`, outside the backend, is
acceptable until the GUI can quit gracefully) and the data directory.

## OS behavior and evidence

| OS | Implemented behavior | Unit-tested here | Not proven |
| --- | --- | --- | --- |
| macOS | Copy the staged ZIP into a private 0700 folder inside a new `.vaio-update-*` staging folder beside the app, re-hash the copy, and run `/usr/bin/ditto -x -k` on the copy; delete the downloaded ZIP; require one app and matching bundle identifier; spawn the current executable in a separate process group as `--apply-update <staged> <target> <pid> <data-dir>`; the caller then quits. The helper first settles any interrupted swap, waits up to 60 seconds for the parent, writes a recovery marker (`update-recovery.txt`) before the first rename, moves the old app to `previous.app`, moves the new app in and relaunches with `open -n`. On any failure it restores the old bundle, relaunches it, and writes `update-error.txt` (what failed, where the old bundle is); if even the restore fails the marker stays so the next helper or app start moves `previous.app` back when the app is missing. After success it removes the error file, `previous.app` and the staging folder. The staging folder is removed on every error path of `install`. If the folder beside the app cannot be created (permission denied) the archive is revealed instead. An unpackaged binary also reveals it. | Argument boundaries, path rules (target must be the running bundle, staged path must be directly in `.vaio-update-*`, bundle identifier match) as pure functions, hidden-argument parsing (only argument 0), wait timeout/error behavior, real filesystem swaps, marker written before the move and cleared after, failed-move/relaunch rollback, restore-failure marker retention, recovery decision and restore, error-file writing, relaunch-after-failure decision, success cleanup (and refusal to delete a non-update folder), copy-and-rehash of a file changed after verification, permission-denied detection, symlink refusal. | Real ZIP extraction, helper detachment/lifecycle, permissions under Applications, Gatekeeper/quarantine, actual LaunchServices relaunch, killed helper and power loss, the app-start recovery call (not wired: needs the data directory in `main.rs`), showing `update-error.txt` in the UI, and the full v0.1.0 to v0.1.1 flow. |
| Windows | Copy the staged installer into a new private folder under the work folder, re-hash the copy, start the copy with the NSIS `/S` flag detached, delete the downloaded original, and return `RestartScheduled`. The copy folder is left in place because the installer is still running from it. | Command builder, copy-and-rehash (shared, tested on macOS). | Windows compilation and runtime (this code was not compiled here), installer format and `/S` compatibility, UAC/SmartScreen, installer completion and relaunch, cleanup of the copy folder. NSIS is an assumption to confirm in Task 12. |
| Ubuntu | Run `pkexec <current exe> --apply-deb <canonical path> <sha256 hex>`. In that root helper mode (argument 0 only) the process creates a new root-owned 0700 directory with `create_dir` (never an existing path), copies the file into it (size capped), hashes the **copy**, refuses a mismatch, runs `/usr/bin/dpkg -i <copy>` as an argument list, and removes the directory. On success the app deletes the downloaded file, starts the new executable and returns `RestartScheduled`. If `pkexec` is absent the folder is revealed with `xdg-open`. | Command builders (no shell, 64 hex digit argument, absolute `dpkg`), argument parsing, copy then hash then run ordering, mismatch refused without running dpkg, cleanup after success and failure, uppercase hex accepted. | Linux compilation and runtime (not compiled here), polkit prompt with an unregistered action, dpkg installation, `pkexec` environment (TMPDIR, PATH, whether the executable path is accepted from an unpackaged location), missing-`pkexec` fallback and relaunch. |

Native execution is gated by `cfg(target_os)`. All three command builders are
compiled and tested on macOS. This does not establish Windows/Linux target builds.
`main` forwards arguments before GPUI startup; helper mode returns a nonzero exit
code on refusal/failure and is disabled in development builds. The backend calls its restart
hook only after `RestartScheduled`; `OpenedInstaller` leaves the app running. Direct
callers of `install_for_current_os(staged, data_dir)` must likewise quit on `RestartScheduled`.

The fallback reveals a selected file in Finder/Explorer, or its containing folder
on Linux. It never executes the downloaded file. No platform has yet been shown to
require fallback in a real installed-release experiment.

## Limits and remaining proof

Copies are made into private directories and hashed there, which closes the window
between verification and use for files swapped by another process of the same user;
the macOS staging folder and extracted bundle are still beside the app and are only
as safe as that directory. The backup is retained if restoration itself fails; such a
failure is returned with its location and written to `update-error.txt`. Filesystem
failure during restoration, a killed helper, and power loss cannot be claimed to
be atomic recovery. A successful `open` exit does not prove the new app stayed alive.

Loopback HTTP tests (tiny_http and one raw TCP server for the stall tests) ran in
the fix round on 127.0.0.1. Direct reader/file tests exercise hashing, size mismatch,
read errors, cleanup, path containment, UTF-8 and manifest caps without sockets. The
redirect tests were mutation-checked: with the policy check disabled they fail. No
test was ignored or deleted; tests whose behavior changed on purpose (helper argument
list, `swap_bundle` data-directory argument, helper only from argument 0) were updated.

No GUI/manual installer test was run: the brief expressly delegates these to
Task 12. No packages were installed, dependencies/configuration changed, or git
commands executed.

## Fix round (security review)

R1 to R8, R10, R12 and R13 are implemented as described above. Not done: calling
`recover_interrupted_update` at app start and showing `update-error.txt` (needs the data
directory in `main.rs`, which is not exposed by `paths.rs` or `app.rs`).

## Command record (first run)

All commands ran in the brief's working directory. Repeated identical commands
are listed once with each observed result. Build output was emitted by the tool;
results below summarize it without claiming a blocked test passed.

```sh
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo test -p vaio-platform update
```

Four runs, all exit 101: (1) E0277 in the new test's SHA-256 formatting; replaced it with the known literal digest. (2) 11 passed, 8 failed: two expected stub failures and six loopback failures. (3) 11 passed, 12 failed: six expected stub failures and six loopback failures. (4) 17 passed, six loopback failures; all direct reader/file tests passed.

```sh
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo test -p vaio-platform update::install_tests
```

Four runs: (1) exit 101, eight expected stub failures; (2) exit 0, eight passed; (3) exit 101, eight passed and the new symlink-refusal regression failed; (4) exit 0, nine passed.

```sh
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo fmt --check
```

Two runs: exit 1 for formatting differences in the new code, then exit 0 with no output after cargo fmt.

```sh
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo fmt
```

Exit 0, no output.

```sh
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo clippy --workspace --all-targets -- -D warnings
```

Exit 0; Finished `dev` profile [unoptimized + debuginfo] target(s) in 13.30s. No warnings.

```sh
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo test --workspace
```

Exit 101. Core 110 + 19 passed; desktop 1 + 5 + 2 + 5 + 1 passed; media 16 passed; platform 50 passed and six loopback failures. Total observed: 209 passed, six blocked failures. Cargo stopped there, so the storage/xtask test binaries and doc-tests were not executed in this run.

```sh
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo xtask check-structure
```

Exit 0. The checker printed no violations (cargo printed its build/run progress on stderr). No new source directory was created, so structure-map regeneration was not required.

The six loopback-dependent failures are:

- `download_writes_to_work_dir_only`
- `truncated_download_is_error`
- `oversized_download_removes_partial_file`
- `fetch_manifest_returns_text`
- `manifest_body_is_capped_and_valid_utf8`
- `redirects_cannot_escape_the_transport_policy`

Read-only inspection commands below each exited 0 and printed the requested
source text or matching lines. Some long outputs were truncated by the tool.

```sh
cat /Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/2026-11-23-sprint-1-2/task-7/brief.md
cat /Users/kienlt/Developer/video-all-in-one/.agents/skills/rust/SKILL.md /Users/kienlt/Developer/video-all-in-one/.agents/skills/rust-async-patterns/SKILL.md /Users/kienlt/Developer/video-all-in-one/.agents/skills/security-and-hardening/SKILL.md /Users/kienlt/Developer/video-all-in-one/.agents/skills/test-driven-development/SKILL.md /Users/kienlt/Developer/video-all-in-one/.agents/skills/structure-codebase/SKILL.md .agents/skills/using-superpowers/SKILL.md
cat /Users/kienlt/Developer/video-all-in-one/.agents/skills/test-driven-development/SKILL.md /Users/kienlt/Developer/video-all-in-one/.agents/skills/rust/references/error-handling.md /Users/kienlt/Developer/video-all-in-one/.agents/skills/rust/references/testing.md
cat crates/core/src/ports/update_backend.rs crates/core/src/update/mod.rs crates/platform/Cargo.toml crates/platform/src/lib.rs apps/desktop/src/main.rs Cargo.toml rust-toolchain.toml docs/architecture.md
sed -n '1,180p' /Users/kienlt/Developer/video-all-in-one/docs/superpowers/plans/2026-11-23-sprint-1-2.md
cat crates/core/src/ports/update_backend.rs crates/core/src/update/mod.rs crates/core/src/update/service.rs crates/platform/Cargo.toml crates/platform/src/lib.rs crates/platform/src/update/mod.rs apps/desktop/src/main.rs Cargo.toml rust-toolchain.toml docs/architecture.md
cat crates/core/src/update/staged.rs crates/core/src/update/error.rs crates/core/src/update/manifest.rs /Users/kienlt/Developer/video-all-in-one/.agents/skills/test-driven-development/writing-good-tests.md
rg -n -i 'updat|install|macos|windows|ubuntu' /Users/kienlt/Developer/video-all-in-one/docs/superpowers/specs/2026-11-23-video-all-in-one-design.md
cat /Users/kienlt/Developer/video-all-in-one/.agents/skills/structure-codebase/references/visible-hexagon.md /Users/kienlt/Developer/video-all-in-one/.agents/skills/security-and-hardening/references/hardening-patterns.md
rg -n 'NSIS|WiX|wix|MSI|msi|silent|Windows.*installer' /Users/kienlt/Developer/video-all-in-one/docs/superpowers/plans/2026-11-23-sprint-1-2.md
cat crates/platform/src/update/install_windows.rs crates/platform/src/update/http.rs
```

## Ripple edits

The existing update module declares/exports the adapters and helper entry point;
main forwards hidden arguments before starting GPUI; the existing architecture
map purpose now includes transport and installation. No other paths were edited.

