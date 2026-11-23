# Video All-in-One: Design

Status: **awaiting approval**. Built from the three ChatGPT design notes (tool comparison, Web vs Desktop, skill and stack proposal). Choices marked *(my call)* were not stated by the user.

## 1. Intent and why

The user cuts local videos, downloads public videos, and downloads only chosen sections of long online videos. Today this means hand-typing `ffmpeg` and `yt-dlp` commands, and downloading a 70-minute video just to keep a few minutes. Goal: one desktop app that does all of it with no command line.

Existing tools cover single parts (LosslessCut for local cuts, yt-dlp GUIs for sections). The difference this app aims for: **one workspace** where a local file or a URL goes in, several segments are marked, and everything is exported in one run.

## 2. Goals, non-goals, anti-goals

**Goals**
- The 10 requirements in section 3 work end to end on Windows 10+, macOS 13+, Ubuntu 22.04+.
- Free, local-first, no account, no watermark, no upload of the user's files.
- Fast cut (stream copy) and precise cut (re-encode) are both available and clearly labeled.

**Non-goals**
- Not a general video editor (no effects, titles, multi-track editing).
- No cloud backend, no server-side processing, no web version.
- No study or note-taking features, no transcript or AI features.
- Windows 7 and Ubuntu 20.04 are not supported.

**Anti-goals** (failure even if it technically works)
- Silently overwriting or deleting the user's source video.
- Claiming frame-accurate output from stream copy.
- A UI that freezes during download or encode.
- A look and feel that is just another generic downloader.

## 3. The 10 requirements

| # | Requirement | Group |
|---|---|---|
| 1 | Local Video Cutting: open a file, choose start and end, cut | Core |
| 2 | Online Video Downloading: public URL (YouTube, Facebook, others yt-dlp supports) | Core |
| 3 | Online Multi-Segment Downloading: several time ranges from one URL, as separate files | Core |
| 4 | Remove Video Sections: delete parts and keep the rest | Core |
| 5 | Project Save & Resume: reopen a project with sources and segments intact | Core |
| 6 | Smart Paste: paste URLs and timestamp lists (`00:01:00 - 00:05:00 Name`) and get segments | Support |
| 7 | Visual Clip Selector: choose segments on a timeline with preview | Support |
| 8 | Multi-Segment Export: separate files or one merged file, named per segment | Support |
| 9 | Batch Processing: many sources and segments in one run | Operations |
| 10 | Persistent Job Queue: jobs survive restart and crash, with retry and cancel | Operations |

## 4. Constraints

- Stack (decided by the user): **Rust, GPUI (Zed) + GPUI Kit, Tokio, SQLite (rusqlite), FFmpeg/FFprobe, yt-dlp**. No React, Tauri, Electron, webview, or Tailwind.
- FFmpeg, FFprobe, and yt-dlp are external executables run as subprocesses. Some yt-dlp extractors need a JavaScript runtime.
- GPUI is pre-1.0. Pin the GPUI version that `gpui-kit` locks; do not mix GPUI sources.
- Online downloading depends on each platform's terms and on yt-dlp working with current sites. The app only processes content the user chooses and is responsible for; it ships no site-specific bypasses. *(my call)*
- yt-dlp section download (`--download-sections`) is not always faster than downloading the whole video and has had bugs with some FFmpeg versions. A full-download-then-cut fallback is required.

## 5. Approaches considered (how it gets built)

| Option | Trade-off | Verdict |
|---|---|---|
| A. GPUI + Rust (user's choice) | Native, no webview, one language. Pre-1.0 framework, no built-in video player, more custom UI work. | **Chosen by the user** |
| B. Tauri 2 + React | Faster UI work, mature ecosystem, native video element. Two languages. | Rejected by the user |
| C. Dioxus Desktop + Tailwind | Rust only, easier styling, still a webview. | Rejected by the user |

Within A, the structure is a **Cargo workspace, modular monolith with ports and adapters** *(my call)*: a UI app crate plus a small number of library crates, not many micro-crates, because there is one developer.

## 6. Architecture

```
GPUI app (UI state only)
   │ commands / events
Application core (Rust)  ── domain, services, ports
   ├── media_planner ── plans cut/download/merge strategy (no I/O)
   ├── job_scheduler ── queue, concurrency, retry, cancel
   ├── project_manager, source_manager, segment_manager, smart_paste
   │
Adapters
   ├── media crate: FFmpeg, FFprobe, yt-dlp, process_runner, thumbnails, cache, verification
   ├── storage crate: SQLite repositories and migrations
   └── platform: file dialogs, OS paths, binary resolution
```

Rules: the UI never runs encode, download, or database calls in an event handler. Background work runs on Tokio and reports progress through events. Planning is separate from execution. One segment model serves keep, remove, and export.

### Workspace layout (UI)

Kept as the user's chosen "Standard Workspace": top toolbar, left sources, center preview, right inspector, bottom timeline, job queue below it, status bar with "Process All". Panels resize; the job queue collapses. The user asked that it not look generic; visual identity (typography, timeline style, theme) follows `gpui-kit-design-guides` and is decided during design of the screens, not here.

### Folder structure *(from the notes, adjusted)*

```
apps/desktop/           GPUI app: ui/, commands/
crates/core/            domain, services, ports, errors
crates/media/           ffmpeg, ffprobe, yt_dlp, process_runner, progress, thumbnails, cache, verification, player adapter
crates/storage/         SQLite repositories, migrations
docs/superpowers/       specs and plans
```
`platform` code lives in `crates/media` or a small `crates/platform` crate; the builder decides *(my call)*.

## 7. Key behaviors

- **Cut modes:** *Fast cut* (stream copy, snaps to keyframes, labeled as such) and *Precise cut* (re-encode, accurate times).
- **Time intervals** are normalized (sorted, clamped, overlaps and adjacent ranges merged) before any processing; time is stored as integers.
- **Online segments:** try partial download first when the source supports it; fall back to full download then cut. Reuse a valid cached download within a batch.
- **Merge** only after validating codec, container, audio streams, and timestamps; otherwise re-encode or tell the user.
- **Outputs** are written to a temp file and renamed after FFprobe validation. Source files are never changed.
- **Jobs** are saved to SQLite before they run. On startup, interrupted jobs are reconciled before new work starts. Cancel kills the whole child process tree. Resume only where the operation allows it.
- **Projects** store sources, segments, export settings, and job history in SQLite.

## 8. Open items and risks

1. **Video preview backend is not chosen.** Candidates: `alhazen-gpui`, `gpui-video-player` (GStreamer), or FFmpeg decode with custom GPUI rendering. They may need a different GPUI version than `gpui-kit`. Decision: build a `VideoPlayerAdapter` trait first, then a **prototype** (play, seek, thumbnail timeline, multi-segment selection) on all three OS before building the full app. This is the first implementation task.
2. **Distribution scope** *(my call)*: first target is the user's own daily use on macOS; Windows and Linux builds, installers, signing, and updates come after the core works. Build code so those stay possible (no hard-coded paths, binary resolution through one module).
3. **Bundling FFmpeg and yt-dlp:** licensing (FFmpeg build flags, GPL vs LGPL) and update strategy are undecided; settle before any public release.
4. **Skill gap:** the notes also list a `web-research` skill; it is not installed. Install or replace before research-heavy tasks.
5. Remaining tool-level choices (progress parsing, thumbnail format, cache size limits, naming templates) are left to the builder.

## 9. Success

The user can paste a long video URL (or open a local file), mark several segments on a timeline or by pasting timestamps, press one button, and get correctly named files, with the UI responsive throughout and the queue intact after closing and reopening the app.

## 10. Next step

After approval: `writing-plans`. Plan task 1 is the preview prototype; tasks that do not depend on the player (core domain, segment normalization, smart paste, SQLite schema, job state machine) can proceed in parallel.
