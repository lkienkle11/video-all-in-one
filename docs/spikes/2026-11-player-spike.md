# Player spike — November 2026

## Candidate: libmpv

Status: **DONE_WITH_CONCERNS**. Debug and release compile. Software rendering and
ten exact seeks ran on the supplied fixture. The sandbox denies the required
`ps` subprocess, so RSS is unavailable and the headless command exits 1 after
printing its other measurements. This is not an overall player pass: the GUI,
sound, displayed-frame performance, and installer have not been exercised.

### Implementation

Standalone `spikes/player-mpv`, with the original manifest and lockfile unchanged:
`gpui-kit =0.11.0`, `libmpv2 7.0.1`. No new dependency or package installation.

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
{"first_frame_ms":32.278,"frames_rendered":302,"avg_fps":30.200,"max_frame_gap_ms":38.303,"seek_ms":[36.794, 19.606, 19.200, 9.690, 170.316, 182.191, 196.513, 16.686, 21.714, 22.239],"seek_max_ms":196.513,"rss_mb":null,"libmpv_version":"mpv v0.42.0"}
```

Stderr and exit status:

```text
player-mpv: ps -o rss= -p 63906: Operation not permitted (os error 1)
```

Exit **1**, specifically because RSS could not be collected. The program invokes
`ps` with `Command::new("ps").args(["-o", "rss=", "-p", pid])`, never a shell
string. On success it reports RSS in MiB (ps KiB divided by 1024). On failure it
prints `null`, preserves the measured playback results, and returns the error.
This nullable field is an explicit sandbox limitation, not a measured zero.
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
196.513 ms). This establishes only the headless render portion of the criterion.
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

| Criterion | Evidence/status |
| --- | --- |
| 1080p source → 1280×720 software render | Measured above |
| Headless exact seek under 500 ms | Ten values above; maximum 196.513 ms |
| RSS | Blocked: sandbox denies spawning `ps` |
| GUI frames, keys and real audio | not run (needs a display) |
| GUI seek-to-visible-frame and responsiveness | not run (needs a display) |
| Concurrent FFmpeg job / thumbnail strip | Not run; reserved for orchestrator |
| Idle CPU | Not measured; display validation and permitted process observation needed |
| Installer size growth | Not measured; binary size alone reported below |
| Other platforms / clean-machine install | Not run |

### Commands and results

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
The final run's complete JSON, stderr and exit status are recorded above.

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

Within allowed paths only: corrected GPUI focus/close callback signatures and
handled atlas-removal errors; replaced fixed `chunks_exact` calls with
`as_chunks::<4>()`/`as_chunks_mut::<4>()` for the installed Clippy. No parent
workspace, dependency, configuration, architecture-map or production-code edits.

No git state/history changes, commit, push, installs, file deletions, GUI launch,
audio-device test, FFmpeg workload, thumbnail test or installer work was done.
The CLI-owned final result file was not created.

### Source basis

- [mpv stable manual](https://mpv.io/manual/stable/): options, seek behavior,
  `playback-time`/`time-pos` and `seeking` properties. Native ABI/threading details
  were checked against the installed `/opt/homebrew/include/mpv/render.h`.
- Cached `libmpv2-7.0.1/src/mpv.rs` and `src/mpv/render.rs`: public handle and the
  wrapper's OpenGL-only API, corresponding to [libmpv2 docs](https://docs.rs/libmpv2/7.0.1/libmpv2/).
- Cached `gpui-kit-0.11.0/src/lib.rs`, `gpui-pre-0.3.14/src/{assets.rs,platform.rs,window.rs,elements/img.rs}`:
  facade/bootstrap and actual image/focus/cache APIs; [GPUI Kit coding guide](https://gpui-kit.com/docs/coding-guides.md).
- Cached `image-0.25.10/src/codecs/bmp/decoder.rs` and
  `smallvec-1.16.6/src/lib.rs`: RGB8 BMP decode and the per-frame clone count.
