# Player spike — November 2026

## Candidate: libmpv

Status: **DONE_WITH_CONCERNS** (updated 2026-11-24 after the orchestrator re-check; see the last two sections, which supersede the single-run numbers above). Continuation verified debug and release builds,
software rendering and ten exact seeks on the supplied fixture. The headless
command now exits **0** and reads peak RSS without spawning a subprocess on
macOS. This is not an overall player pass: the GUI,
sound, displayed-frame performance, and installer have not been exercised.

### Implementation

Standalone `spikes/player-mpv`: `gpui-kit =0.11.0`, `libmpv2 7.0.1`.
The continuation adds a macOS-only direct dependency on `libc =0.2.191`, already
present in this spike's lockfile and cache. Cargo updates only the spike package's
dependency list in the lockfile; no new crate/version or package installation.

- `main.rs`: CLI, canonicalized local fixture path, mode selection, error exit.
- `software.rs`: libmpv initialization and a narrow software-render FFI boundary.
- `render_thread.rs`: native render ownership, callback wake-ups, shutdown.
- `headless.rs`: timing window, exact seek measurements, JSON and RSS collection.
- `frames.rs`: GUI control worker, bounded frame transfer, image conversion.
- `gui.rs`: GPUI Kit bootstrap, retained image, keyboard handling and time/FPS text.

Both modes set `config=no`, `hwdec=auto-safe`, `vo=libmpv`, `terminal=no` and
`keep-open=yes`. Only headless sets `ao=null`; GUI leaves audio output at the
default. Native software rendering uses API type `sw`, a 64-byte-aligned
1280×720 RGBA buffer, and a 5120-byte row stride. No window is created headless.

### Measured headless result

Working directory for all Cargo/build/run commands below:

```text
/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/spikes/player-mpv
```

Exact final measurement command (run after compilation finished):

```sh
./target/release/player-mpv --fixture /Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/spike-fixtures/fixture-1080p30-10min.mp4 --headless
```

Stdout, exactly one JSON line:

```json
{"first_frame_ms":55.284,"frames_rendered":302,"avg_fps":30.200,"max_frame_gap_ms":40.753,"seek_ms":[21.549, 21.938, 123.552, 168.791, 179.121, 25.800, 26.033, 12.133, 15.830, 176.422],"seek_max_ms":179.121,"rss_mb":225.500,"rss_error":null,"libmpv_version":"mpv v0.42.0"}
```

Stderr: empty. Exit **0**. On macOS, `getrusage(RUSAGE_SELF).ru_maxrss`
reports peak resident bytes, converted to MiB by dividing by 1,048,576. It is
sampled after the seeks; it is the process lifetime peak, not instantaneous RSS
or GUI memory usage. This intentionally changes the old `ps` measurement's
semantics, as permitted by the continuation brief. Other targets retain
`Command::new("ps").args(["-o", "rss=", "-p", pid])` and KiB-to-MiB conversion.
An RSS failure now emits `rss_mb: null` and a JSON-escaped `rss_error` reason;
successful telemetry emits `rss_error: null`. It never fails otherwise successful
playback. Playback/render/seek errors still fail the command. The RSS failure
branch was inspected but not forced in this continuation's runtime check.
The render API and fixture loading were not blocked. No real audio device was
attempted in this run.

The first-frame clock starts immediately before `loadfile`; render timestamps
are captured on the native render thread. The playback window is ten seconds
starting at its first rendered picture, including that first picture.
`frames_rendered` counts rendered pictures, including native redraw/repeat
requests; it is not a count of unique decoded frames or GPUI presentations.
The reported maximum gap includes the final boundary gap of the window.

Seeks are paused, `absolute+exact`, to `duration × i / 11`, for i=1..10:
approximately 54.545, 109.091, 163.636, 218.182, 272.727, 327.273, 381.818,
436.364, 490.909 and 545.455 seconds for this 600-second fixture. Pre-seek
notifications are drained. A completion must have a post-command render
timestamp, sampled `playback-time` within 100 ms of target, and `seeking=false`.
This uses the player's reported playback time after rendering; it does not
independently decode a timestamp from the image pixels. Pausing prevents the
target from advancing while the control thread samples it. Each phase has a
15-second wait deadline.

All ten measured software-render seek latencies were below 500 ms (maximum
179.121 ms). This establishes only the headless render portion of the criterion.
It does not establish command-to-displayed-frame latency in a GPUI window.

### Frame hand-off and copy cost

1. The `mpv-render` thread owns the software context and buffer. The native update
   callback only calls `Thread::unpark`; it makes no mpv calls. The thread parks
   when idle. Synchronous commands/property reads run on a separate control
   thread, never on GPUI or on the native render thread.
2. In GUI mode, the render thread turns RGBA into an uncompressed top-down BMP,
   then calls the real `gpui_kit::Image::from_bytes(ImageFormat::Bmp, bytes)` and
   `Image::to_image_data(SvgRenderer)` APIs. BMP decoding happens off the UI
   thread. This yields `Arc<gpui_kit::RenderImage>` in GPUI's expected BGRA format.
3. A two-entry bounded channel transfers image Arcs to the control worker. A
   full channel drops the incoming frame rather than blocking native rendering.
   The control worker publishes to a single latest-frame slot and wakes the
   stored Rust task Waker. A retained `cx.spawn_in` task receives it, replaces the
   view's image, evicts the previous sprite-atlas image via `Window::drop_image`,
   and calls `cx.notify()`. The view draws `img(image).object_fit(Contain)`.
   The UI slot replaces obsolete frames when the UI cannot keep up.
4. Space queues pause/resume, Left/Right queue relative exact ±5-second seeks,
   and Escape quits. Current playback seconds and the control worker's measured
   received-frame rate appear as text. That rate is not a display-vsync counter.
   Window close shuts down and joins both workers; normal key handlers only
   queue work and wake the control thread.

There is no sleep-based polling loop. Native callbacks, frame delivery, UI
commands and task Wakers drive progress. Headless uses blocking channel waits
with measurement deadlines; its channel carries timestamps rather than pixels.

Copy accounting from the pinned sources (excluding native decode/scaler internals):

| Stage after native render | Pixel storage written per frame |
| --- | --- |
| RGBA → top-down BMP BGRX | One 3,686,400-byte pixel copy, channel-swizzled |
| GPUI/image BMP decoder → RGB8 | One 2,764,800-byte decoded image |
| `DynamicImage::into_rgba8()` | One 3,686,400-byte converted image |
| `SmallVec::from_elem(Frame::new(data), 1)` | One 3,686,400-byte clone in the pinned implementation |
| Thread and view hand-off | Zero pixel copies; Arc moves/clones |

Thus the GUI path materializes four full images after rendering: three at four
bytes/pixel and one at three bytes/pixel. GPUI also swaps R/B in place, hashes
the BMP bytes, and uploads pixels to its GPU atlas. GPU/driver copies were not
measured. Headless performs none of this BMP/GPUI conversion. This deliberately
uses only the supplied dependencies and has a meaningful CPU/bandwidth cost;
headless FPS must not be presented as proof of GUI throughput.

### Compatibility and risks

No dependency-version conflict between GPUI Kit and libmpv2 was found: both
debug and release link successfully together. GPUI Kit uses `gpui-pre 0.3.14`.
The initial GUI compile needed two local API corrections: `focus(window, cx)`
and the two-argument `on_window_closed` closure.

The libmpv2 7.0.1 high-level render API supports OpenGL only. Software rendering
uses its public `Mpv::ctx` handle with a small, documented C ABI bridge matching
the installed `/opt/homebrew/include/mpv/render.h`. This is not a crate patch or
an additional sys dependency. The bridge owns its callback and aligned buffer,
serializes render calls, and frees its render context before the Mpv handle.
Native control and render calls are separated to satisfy the header's threading
restrictions. FFI still needs platform/lifecycle review before production use.
The software `rgba` format worked with the installed mpv v0.42.0; other bundled
versions/platforms must verify format support and fallback behavior separately.

The native render-frame flags identify paused seek pictures as redraws in this
run. Excluding redraws caused a measured seek timeout; the final implementation
includes them and keeps timestamp/position/seek-state validation.

The binary dynamically links Homebrew libmpv. Its file size is not installer
growth: libmpv and its transitive dylibs, redistribution licences, packaging and
relocation have not been accounted for. `hwdec=auto-safe` is an option request,
not evidence that hardware decoding was selected. Large/random-GOP fixtures,
audio synchronization, UI stalls, long-running memory use, resize, color fidelity,
EOF behavior, and clean-machine Windows/Linux/macOS deployment remain risks.

| Criterion | Result | Evidence | Status |
| --- | --- | --- | --- |
| Debug and release compile, including GUI | Pass | Continuation build commands below exit 0 | measured |
| Headless command succeeds | Pass | One JSON line above, exit 0, no stderr | measured |
| 1080p30 source → 1280×720 software render | 302 renders / 10 s; 30.2 renders/s; first frame 98-105 ms; maximum gap 38-39 ms (orchestrator, 3 runs; worker's single run: 55 ms / 40.8 ms) | Release fixture run above | measured |
| Headless exact seek under 500 ms | Pass on the 30 fps fixture (worst 203 ms, 3 runs). **Fails on 24 fps (553 ms)**; passes on 60 fps (341 ms) and VFR (427 ms). Latency is the time until mpv reports the target and clears `seeking`; it is not pixel-verified | Frame-rate check table | measured |
| Peak RSS (macOS getrusage high-water mark, MiB) | 144-165 MiB over 6 runs (worker's single run: 225.5) | macOS getrusage; rss_error=null | measured |
| RSS-unavailable fallback | Source handles null and reason without failing playback | Failure branch inspected; no injected failure | not run |
| Moving video with real sound in GPUI | User confirmed picture and sound on 2026-11-24 (by eye and ear, not by metrics) | GUI compiles; headless uses ao=null | needs a display |
| Space, Left/Right, Escape, time/FPS text | not run (needs a display) | Hand check below | needs a display |
| GUI seek shows new frame under 500 ms | not run (needs a display) | Headless timing excludes image conversion, upload and presentation | needs a display |
| Smooth playback and resize | not run (needs a display) | BMP conversion and atlas costs unmeasured | needs a display |
| UI responsive during FFmpeg job | Deferred to orchestrator | No FFmpeg workload run | needs a display |
| At least 500 thumbnails scroll smoothly | Deferred to orchestrator | No thumbnail strip built in this candidate | needs a display |
| Idle CPU | Not measured | Requires real paused window and process observation | needs a display |
| Release executable size | 20,280,576 bytes (19.34 MiB) | Continuation ls output below | measured |
| Installer size growth | Not measured | Dylib dependency closure and installer not built | not run |
| Windows/Linux and clean-machine macOS | Not run | Only this macOS sandbox build/run | not run |

### Repeatable display check for the orchestrator

From `spikes/player-mpv`, run the freshly built release binary without `--headless`:

```sh
./target/release/player-mpv --fixture /Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/spike-fixtures/fixture-1080p30-10min.mp4
```

This command was **not run (needs a display)**. On a machine with Homebrew libmpv
and an enabled audio output:

1. Observe at least 30 seconds of moving video and listen to the fixture's audio.
   Record sound availability, interruptions and any observable A/V drift. Check
   orientation, color and aspect ratio. Record OS, display refresh rate and scale.
2. Confirm seconds advance and FPS text settles near the source's 30 fps. Record
   the range and visible stutter over 60 seconds. The label counts frames received
   by the control worker, not actual screen presentations; visual smoothness must
   be checked separately.
3. Press Space: video, time and audio should pause. Press Space again: all resume.
   While paused, FPS text can retain its last sample; do not treat it as idle CPU.
4. Press Right and Left several times both playing and paused: each seeks ±5 s
   (subject to file boundaries), updates picture/time, and keeps keys responsive.
   Record command-to-new-visible-frame latency with a timestamped screen/input
   recording; require every tested seek below 500 ms. Headless numbers alone do
   not satisfy this check. Resume and verify sound follows the new position.
5. Resize repeatedly while playing and check for freezes, corruption, stutter or
   lost audio. Pause for 30 seconds and record idle CPU with Activity Monitor;
   separately observe memory over several minutes of playback.
6. Press Escape and confirm the window and process exit. Relaunch and close the
   window normally to check worker shutdown. Record any stderr/errors or hang.
7. The orchestrator separately runs the concurrent FFmpeg workload and the
   500-thumbnail strip checks, then measures packaged installer growth including
   native libraries and repeats platform checks. Those are outside this worker.

### Continuation commands and results (run 2)

All commands below ran from the standalone spike directory. Each ran once and
exited 0. Output is preserved as printed, including the debug SDK warning.

```sh
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true LIBRARY_PATH=/opt/homebrew/lib PKG_CONFIG_PATH=/opt/homebrew/lib/pkgconfig PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo fmt --check
```

No output.

```sh
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true LIBRARY_PATH=/opt/homebrew/lib PKG_CONFIG_PATH=/opt/homebrew/lib/pkgconfig PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo build
```

```text
   Compiling player-mpv v0.0.0 (/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/spikes/player-mpv)
warning: output of `xcrun` while finding MacOSX.sdk
  |
  = note: 2026-11-24 08:04:41.934 xcodebuild[9442:35549]  DVTFilePathFSEvents: Failed to start fs event stream.
          2026-11-24 08:04:42.139 xcodebuild[9442:35547] [MT] DVTDeveloperPaths: Failed to get length of DARWIN_USER_CACHE_DIR from confstr(3), error = Error Domain=NSPOSIXErrorDomain Code=5 "Input/output error". Using NSCachesDirectory instead.
          2026-11-24 08:04:42.514 xcodebuild[9473:35598]  DVTFilePathFSEvents: Failed to start fs event stream.
          2026-11-24 08:04:42.628 xcodebuild[9473:35597] [MT] DVTDeveloperPaths: Failed to get length of DARWIN_USER_CACHE_DIR from confstr(3), error = Error Domain=NSPOSIXErrorDomain Code=5 "Input/output error". Using NSCachesDirectory instead.

warning: `player-mpv` (bin "player-mpv") generated 1 warning
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 5.74s
```

```sh
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true LIBRARY_PATH=/opt/homebrew/lib PKG_CONFIG_PATH=/opt/homebrew/lib/pkgconfig PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo build --release
```

```text
   Compiling player-mpv v0.0.0 (/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/spikes/player-mpv)
    Finished `release` profile [optimized] target(s) in 2.13s
```

```sh
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true LIBRARY_PATH=/opt/homebrew/lib PKG_CONFIG_PATH=/opt/homebrew/lib/pkgconfig PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo clippy --workspace --all-targets -- -D warnings
```

```text
    Checking player-mpv v0.0.0 (/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/spikes/player-mpv)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 1.04s
```

```sh
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true LIBRARY_PATH=/opt/homebrew/lib PKG_CONFIG_PATH=/opt/homebrew/lib/pkgconfig PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo test --workspace
```

```text
   Compiling player-mpv v0.0.0 (/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/spikes/player-mpv)
    Finished `test` profile [unoptimized + debuginfo] target(s) in 0.75s
     Running unittests src/main.rs (target/debug/deps/player_mpv-f529b53da821401c)

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

```sh
ls -l target/release/player-mpv
```

```text
-rwxr-xr-x@ 1 kienlt  staff  20280576 Nov 24 08:04 target/release/player-mpv
```

The exact release headless command and its complete output appear in "Measured
headless result" above; it ran once after compilation and tests finished.

#### Continuation read-only command ledger

These ran from the working-folder root. Every invocation exited 0 and printed
the requested source text or matching lines. Large batched tool output was
truncated in the transcript; focused reads below recovered the affected source,
draft and required constraints. No read command mutated files.

```sh
cat /Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/2026-11-23-sprint-1-2/task-3a/brief.md
cat /Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/.agents/skills/using-superpowers/SKILL.md
cat /Users/kienlt/Developer/video-all-in-one/.agents/skills/rust/SKILL.md /Users/kienlt/Developer/video-all-in-one/.agents/skills/gpui-kit/SKILL.md /Users/kienlt/Developer/video-all-in-one/.claude/skills/gpui-video-timeline/SKILL.md /Users/kienlt/Developer/video-all-in-one/.agents/skills/rust-async-patterns/SKILL.md /Users/kienlt/Developer/video-all-in-one/.claude/skills/web-research/SKILL.md
cat spikes/player-mpv/src/frames.rs spikes/player-mpv/src/gui.rs spikes/player-mpv/src/headless.rs spikes/player-mpv/src/main.rs spikes/player-mpv/src/render_thread.rs spikes/player-mpv/src/software.rs
cat docs/spikes/2026-11-player-spike.md
cat /Users/kienlt/Developer/video-all-in-one/docs/superpowers/plans/2026-11-23-sprint-1-2.md /Users/kienlt/Developer/video-all-in-one/docs/superpowers/specs/2026-11-23-video-all-in-one-design.md
cat spikes/player-mpv/src/headless.rs spikes/player-mpv/src/software.rs spikes/player-mpv/src/main.rs spikes/player-mpv/Cargo.toml
cat spikes/player-mpv/src/frames.rs spikes/player-mpv/src/gui.rs spikes/player-mpv/src/render_thread.rs docs/spikes/2026-11-player-spike.md
cat /Users/kienlt/Developer/video-all-in-one/.claude/skills/gpui-video-timeline/SKILL.md /Users/kienlt/Developer/video-all-in-one/.agents/skills/rust-async-patterns/SKILL.md /Users/kienlt/Developer/video-all-in-one/.claude/skills/web-research/SKILL.md
sed -n '/^## Global Constraints/,/^## /p' /Users/kienlt/Developer/video-all-in-one/docs/superpowers/plans/2026-11-23-sprint-1-2.md
rg -n -A 7 '^name = "libc"|^name = "player-mpv"' spikes/player-mpv/Cargo.lock
cat /Users/kienlt/Developer/video-all-in-one/.agents/skills/rust/references/error-handling.md /Users/kienlt/Developer/video-all-in-one/.agents/skills/rust/references/unsafe-and-ffi.md /Users/kienlt/Developer/video-all-in-one/.agents/skills/rust/references/crates-and-dependencies.md /Users/kienlt/Developer/video-all-in-one/.agents/skills/gpui-kit/references/coding-guides.md
rg -n 'pub fn getrusage|pub struct rusage|ru_maxrss|RUSAGE_SELF' target/cargo-home/registry/src/*/libc-0.2.191/src/unix/bsd/apple/mod.rs target/cargo-home/registry/src/*/libc-0.2.191/src/unix/mod.rs
sed -n '100,130p' target/cargo-home/registry/src/*/libc-0.2.191/src/unix/mod.rs
cat spikes/player-mpv/src/headless.rs spikes/player-mpv/Cargo.toml
rg -n -A 7 '^name = "libc"|^name = "player-mpv"' spikes/player-mpv/Cargo.lock
sed -n '1,82p' docs/spikes/2026-11-player-spike.md
sed -n '150,340p' docs/spikes/2026-11-player-spike.md
```

The first lockfile query confirmed libc 0.2.191 already existed; the second
confirmed the added direct dependency. The final reads confirmed optional RSS,
the exact recorded JSON and the display checklist/table. Edits used apply_patch,
not shell commands. Official web reads: Apple's archived getrusage manual and
current XNU manual succeeded; docs.rs getrusage returned an accessibility error.
The copy-cost account is retained from the prior pinned-source inspection;
this continuation reread all six spike source files and changed no GUI pipeline.

### Prior run commands and results (historical)

The following records belong to the previous run, before the RSS fix. Their
sizes, timings and exit-1 outcomes are superseded by the continuation above.

Every Cargo invocation used this full prefix, without rtk. These are the exact
distinct verification commands executed, all from the standalone spike directory:

```sh
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true LIBRARY_PATH=/opt/homebrew/lib PKG_CONFIG_PATH=/opt/homebrew/lib/pkgconfig PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo fmt
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true LIBRARY_PATH=/opt/homebrew/lib PKG_CONFIG_PATH=/opt/homebrew/lib/pkgconfig PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo fmt --check
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true LIBRARY_PATH=/opt/homebrew/lib PKG_CONFIG_PATH=/opt/homebrew/lib/pkgconfig PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo build
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true LIBRARY_PATH=/opt/homebrew/lib PKG_CONFIG_PATH=/opt/homebrew/lib/pkgconfig PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo build --release
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true LIBRARY_PATH=/opt/homebrew/lib PKG_CONFIG_PATH=/opt/homebrew/lib/pkgconfig PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo clippy --workspace --all-targets -- -D warnings
CARGO_HOME=/Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/cargo-home CARGO_NET_OFFLINE=true LIBRARY_PATH=/opt/homebrew/lib PKG_CONFIG_PATH=/opt/homebrew/lib/pkgconfig PATH=/opt/homebrew/opt/rustup/bin:$PATH cargo test --workspace
ls -l target/release/player-mpv
```

| Command | Runs and results |
| --- | --- |
| `cargo fmt` | Four runs; exit 0, no output |
| `cargo fmt --check` | Three runs; exit 0, no output |
| `cargo build` | Six runs: initial exit 101 (E0061 focus context; E0593 close callback arity); five subsequent exits 0. Final `Finished dev profile [unoptimized + debuginfo] target(s) in 0.97s` |
| `cargo build --release` | Four runs; exit 0 each. Final `Finished release profile [optimized] target(s) in 1.02s` |
| `cargo clippy --workspace --all-targets -- -D warnings` | Three runs: initial exit 101 for two `chunks_exact_to_as_chunks` findings; both later exits 0. Final dev-profile finish in 0.80s |
| `cargo test --workspace` | Two runs; exit 0, zero tests; no tests skipped or removed |
| `ls -l target/release/player-mpv` | Three runs; exit 0. Intermediate sizes 20,283,888 and 20,283,552 bytes; final output below |

Final test output:

```text
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

The brief exempts this throwaway spike from TDD/unit-test and structure-map
requirements. `--workspace` above targets the spike's empty `[workspace]`, not
the application's parent workspace. No production test suite claim is made.

Final size output:

```text
-rwxr-xr-x@ 1 kienlt  staff  20283568 Nov 23 22:07 target/release/player-mpv
```

Final release binary: **20,283,568 bytes** (approximately 19.34 MiB).

Earlier runtime command:

```sh
./target/debug/player-mpv --fixture /Users/kienlt/Developer/video-all-in-one/.superpowers/diff-agents/worktrees/2026-11-23-sprint-1-2/w1/target/spike-fixtures/fixture-1080p30-10min.mp4 --headless
```

Executed twice; both exited 1 and initially produced no JSON. First stderr:
`player-mpv: Operation not permitted (os error 1)`. After adding context and
separating the render thread, second stderr:
`player-mpv: ps -o rss= -p 61334: Operation not permitted (os error 1)`.
The first debug run overlapped release compilation; it is not used for numbers.

The release headless command was executed twice. The first exited 1 with no JSON:
`player-mpv: seek to 54.545455s timed out without a matching rendered frame`.
The prior final run exited 1 after printing playback data, because its `ps`
subprocess was denied. Its maximum seek was 196.513 ms and RSS was null.

Read-only inspection used `cat`, `sed`, `rg` and `ls` for the brief, named skills
and linked references, plan/spec, existing bootstrap, supplied manifest/lock,
cached crate sources and installed native header. The initial `ls -ld` confirmed
the fixture and lockfile existed; it exited 1 because this findings file did not
yet exist. One `rg` command exited 1 because the shell glob `libmpv2-7.0.1/src/render*`
had no match; the wrapper was then read at its actual `src/mpv/render.rs` path.
Other read-only commands returned source text/matches or empty selected ranges.
Only official web documentation was accessed; the docs.rs request failed, so
the exact cached libmpv2 source was used. No package or network dependency fetch.

### Ripple edits

Continuation: `spikes/player-mpv/Cargo.toml` exposes the already-locked libc
version on macOS; Cargo adds `libc` to player-mpv's dependencies in Cargo.lock.
No other package version changes. Only `src/headless.rs` and this candidate
section otherwise changed. GUI/source hand-off files were read, not modified.

Prior run:
Within allowed paths only: corrected GPUI focus/close callback signatures and
handled atlas-removal errors; replaced fixed `chunks_exact` calls with
`as_chunks::<4>()`/`as_chunks_mut::<4>()` for the installed Clippy. No parent
workspace, dependency, configuration, architecture-map or production-code edits.

No git state/history changes, commit, push, installs, file deletions, GUI launch,
audio-device test, FFmpeg workload, thumbnail test or installer work was done.
The CLI-owned final result file was not created.

### Source basis

- [Apple XNU getrusage manual source](https://raw.githubusercontent.com/apple-oss-distributions/xnu/main/bsd/man/man2/getrusage.2):
  current Darwin peak RSS uses bytes. Apple's older archived HTML manual says
  kilobytes; the current XNU source resolves that discrepancy. The libc 0.2.191
  cached declarations supply the native struct and function signature. The
  docs.rs 0.2.191 getrusage page was inaccessible in this continuation.
- [mpv stable manual](https://mpv.io/manual/stable/): options, seek behavior,
  `playback-time`/`time-pos` and `seeking` properties. Native ABI/threading details
  were checked against the installed `/opt/homebrew/include/mpv/render.h`.
- Cached `libmpv2-7.0.1/src/mpv.rs` and `src/mpv/render.rs`: public handle and the
  wrapper's OpenGL-only API, corresponding to [libmpv2 docs](https://docs.rs/libmpv2/7.0.1/libmpv2/).
- Cached `gpui-kit-0.11.0/src/lib.rs`, `gpui-pre-0.3.14/src/{assets.rs,platform.rs,window.rs,elements/img.rs}`:
  facade/bootstrap and actual image/focus/cache APIs; [GPUI Kit coding guide](https://gpui-kit.com/docs/coding-guides.md).
- Cached `image-0.25.10/src/codecs/bmp/decoder.rs` and
  `smallvec-1.16.6/src/lib.rs`: RGB8 BMP decode and the per-frame clone count.

## Orchestrator re-check (2026-11-24)

- The worker's headless run passed in its sandbox, but the same release binary failed on the orchestrator's machine in 4 of 4 runs with `seek ... timed out without a matching rendered frame`. Cause: a frame can render before mpv clears the `seeking` flag, and while paused no later frame follows. Fix in `spikes/player-mpv/src/headless.rs`: after a matching frame, poll `seeking` every 1 ms until it clears (bounded by the 15 s deadline).
- After the fix, 3 of 3 runs exit 0: first frame 98-105 ms, 302 frames in 10 s (30.2 fps), max frame gap 38-39 ms, ten exact seeks 30-203 ms (worst 203 ms), RSS 144-160 MB, libmpv v0.42.0 (these numbers replace the worker's single run).
- GUI mode (`player-mpv --fixture <path>`) ran 25 s on a real display without a crash or stderr output (killed by `timeout`). The orchestrator could not see the window or hear the audio, so on-screen smoothness and sound stay **not verified** until the user confirms by eye and ear.

## Frame-rate check (2026-11-24)

The 30 fps in earlier numbers is the fixture's own rate (ffprobe `r_frame_rate = 30/1`), not a fixed value in the code. To prove it, three 1080p 20 s files with sound were generated in `target/spike-fixtures/` (ffmpeg `testsrc2` + sine) and run through the headless command (release, 10 s of playback, 10 exact seeks):

| File | Stream rate | Frames in 10 s | avg_fps | max frame gap | seek max | RSS | exit |
|---|---|---|---|---|---|---|---|
| f24.mp4 | 24/1 | 242 | 24.2 | 46.8 ms | 553 ms | 165 MB | 0 |
| f60.mp4 | 60/1 | 602 | 60.2 | 20.5 ms | 341 ms | 152 MB | 0 |
| vfr.mp4 | 60 for 5 s, then 24 | 422 | 42.2 | 54.8 ms | 427 ms | 156 MB | 0 |

Result: the render rate follows the file (24, 60, and the variable-rate file gives 300 + 5 s x 24 = 420 expected frames, 422 measured). Exact seeks are slower on these files than on the 30 fps one (up to 553 ms against 203 ms) because the test files use ffmpeg's default long GOP; a seek must decode from the previous keyframe. This is a measurement for the player decision, not a failure. Not tested: real-world codecs (HEVC, VP9, AV1), 4K, hardware decode.

## Reviewer findings and limits (2026-11-24)

An independent read-only review (Sonnet 5.5) found no memory-safety bug and no lost wakeup. Limits it asked to state, all accepted:
- First frame is timed from `loadfile`, not from process start or a visible window.
- fps counts the first frame plus every later render over 10 s (about 0.1 fps high); renders include redraws.
- Seek latency is not pixel-verified: mpv may report the target as `playback-time` before the new frame is decoded, so short seeks (12-26 ms) may be understated. The doc does not claim exactness.
- RSS is a peak in MiB on macOS; the `ps` path on other systems would be current RSS, so the two are not comparable.
- The explanation "long GOP" for the slower seeks is a guess: no GOP length or `hwdec-current` was probed. Codec of the fixtures is H.264 (libx264); whether VideoToolbox was used is unknown.
- The picture is always rendered at a fixed 1280x720 surface, not at the real window size. `target_time` and `report_swap` are not used, so display-sync timing is untested.
- Fixed after the review: a failed `playback-time` read no longer drops the only matching frame (retried up to 50 x 2 ms). The 1 ms `seeking` poll is a sleep loop, contrary to the earlier "no sleep-based polling" sentence.

## Decision (2026-11-24, user-approved)

**libmpv is the player engine for macOS, tentatively.** Evidence: real picture and sound confirmed by the user; frame rate follows the file (24, 30, 60, variable); headless exact seeks 30-553 ms; 144-165 MiB peak. Open conditions:
1. Windows and Linux packaging of libmpv (size, licence: mpv is GPL by default, an LGPL build is needed for a non-GPL app) is checked in CI in Task 1R-b. If it cannot be packaged on one OS, switch to the FFmpeg-plus-audio candidate (Task 3b); do not fall back to still images.
2. Re-measure with real 1080p and 4K files, real codecs, and a surface that follows the window size.
3. Seeks on long-GOP files above 500 ms (24 fps file: 553 ms) are a known risk for the timeline.

Rejected for now: `alhazen-gpui` (needs gpui 0.2.2, the pinned GPUI Kit uses gpui-pre 0.3.14, so its views cannot be embedded); FFmpeg libraries plus an audio crate (kept as the fallback).

The port is `VideoPlayer` in `crates/core/src/ports/player.rs` (RGBA `Frame`, exact `seek`). The libmpv adapter in `crates/media/src/player/` is not written yet: it needs the `libmpv2` crate, which the orchestrator must fetch first.
