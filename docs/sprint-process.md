# Sprint process

**Tóm tắt (tiếng Việt):** Dự án chia đúng 4 sprint, mỗi sprint kết thúc bằng một bản phát hành mà các máy đã cài sẽ tự cập nhật (từ v0.2.0). Sprint 1 làm khung phát hành và tự cập nhật trước. File này là nơi theo dõi tiến độ; thiết kế đầy đủ nằm trong spec, các bước chi tiết của sprint 1 và 2 nằm trong plan.

Sources of truth:
- Design: `docs/superpowers/specs/2026-11-23-video-all-in-one-design.md`
- Sprints 1 and 2 plan: `docs/superpowers/plans/2026-11-23-sprint-1-2.md`
- Skill selection for this work: `docs/usp-select-skills/2026-11-23-video-all-in-one-skills.md`

Rules for every sprint:
- Work happens on a branch, never on `main`. Merge, push and tag need the user's permission.
- Each task is test-first. A sprint is done only when its done-criterion is checked by running it, not by reading code.
- Each sprint ends with a release. Before tagging, assess the release size and ask the user for the version number (see `CLAUDE.md`, Release Tag Rule).
- Status values: `todo`, `in progress`, `done`, `blocked`. Update this file when a status changes.

## The 10 requirements

| ID | Requirement | Sprint |
|----|-------------|--------|
| F01 | Local Video Cutting | 2 |
| F02 | Online Video Downloading | 3 |
| F03 | Online Multi-Segment Downloading | 3 |
| F04 | Remove Video Sections | 3 |
| F05 | Project Save & Resume | 4 |
| F06 | Smart Paste | 4 |
| F07 | Visual Clip Selector | 3 |
| F08 | Multi-Segment Export | 2 |
| S01 | Batch Processing | 2 (basic), 4 (complete) |
| S02 | Persistent Job Queue | 2 |

Cross-cutting from sprint 1: release pipeline, self-written auto-update, dev/production split, English and Vietnamese UI, shared modal behavior, README and GitHub About, at least 85% Rust.

---

## Sprint 1: Foundation, release and auto-update

Release: v0.1.0, then v0.1.1 to prove the update. Status: `todo`.

| # | Work | Status |
|---|------|--------|
| 1.1 | Cargo workspace; empty GPUI window builds on three OS | done |
| 1.1b | Architecture map and placement rules (`docs/architecture.md`) and `cargo xtask check-structure` (crate dependency direction, forbidden names, file and folder size, map must match the real tree) | done |
| 1.2 | Lock GPUI and GPUI Kit versions | done |
| 1.3 | Player spike: libmpv chosen for macOS, tentative (Windows and Linux packaging checked in CI); the adapter is built and tested | done (conditions open: see `docs/spikes/2026-11-player-spike.md`) |
| 1.4 | Dev and production channel split | done (release workflow builds with `--features production`) |
| 1.5 | `set-version` and tag checks (Rust `xtask`) | done |
| 1.6 | Signing key pair and GitHub Secrets | blocked: needs the user (`cargo xtask keygen`, secret, repository variable, tag protection; steps in `docs/RELEASING.md`) |
| 1.7 | Release workflow and `latest.json` | in progress (workflow written, not run on GitHub) |
| 1.8 | Updater module and per-OS install steps | in progress (service, HTTP backend, install code and the app wiring done; a real update on each OS not proven; needs the signing key) |
| 1.9 | Settings: version and update states, update dialog | done (connected to the updater in the production channel; refuses until a real public key is set) |
| 1.10 | Bilingual CHANGELOG and `docs/RELEASING.md` | done |
| 1.11 | CI on three OS plus the 85% Rust check | in progress (workflow written, not run) |
| 1.12 | Localization layer, language modal on first launch, language setting, shared modal behavior | done (manual look in a window not done) |
| 1.13 | English `README.md` | done (licence file not created yet) |
| 1.14 | GitHub About (description, topics), with permission | todo |

Done when: v0.1.0 is installed from the published installer; tag v0.1.1 is pushed; the installed app finds it, downloads, verifies, installs and restarts. The development build never calls the update endpoint. The player spike result is written down.

## Sprint 2: Media base, durable job queue, local cutting

Release: v0.2.0. Status: `todo`.

| # | Work | Status |
|---|------|--------|
| 2.1 | Binary resolver (configured folder, then `PATH`) with a clear missing-tool message | done |
| 2.2 | Process runner: spawn, progress, cancel including children, no orphans | done (Windows job object unproven) |
| 2.3 | SQLite: WAL, first migration, backup before migrate, refuse newer schema | done |
| 2.4 | Persistent Job Queue (S02): states, transitions, persist before run, recover after restart, retry, cancel, limits | done (kill test passes) |
| 2.5 | Source analysis with FFprobe | done |
| 2.6 | Workspace shell: toolbar, sources, preview, inspector, timeline placeholder, job queue panel | done (manual look not done) |
| 2.7 | Local Video Cutting (F01) with segment normalization | done (manual look not done) |
| 2.8 | Multi-Segment Export (F08): separate or merged, Fast and Precise cut, export verification | done |
| 2.9 | Basic Batch (S01) | done (manual look not done) |

Done when: three segments of a local video export merged into one correct file; the app is killed mid-job and reopened, and the job shows as failed with a readable reason while the queue still works; installs of v0.1.x update to v0.2.0 by themselves.

## Sprint 3: Online sources, removing sections, visual timeline

Release: v0.3.0. Status: `todo`. Detailed plan to be written after sprint 2.

| # | Work | Status |
|---|------|--------|
| 3.1 | Online Video Downloading (F02): metadata, quality choice, full download with progress | todo |
| 3.2 | Online Multi-Segment Downloading (F03): partial download when supported, always a full-download fallback, no double download in one batch | todo |
| 3.3 | Remove Video Sections (F04), same segment model as keep | todo |
| 3.4 | Real video preview using the player chosen in the sprint 1 spike | todo |
| 3.5 | Visual Clip Selector (F07): thumbnails, playhead, zoom, seek, drag handles, Keep/Remove mode, Boundary Preview | todo |
| 3.6 | Preview for online sources (decide in the plan: low-quality proxy download or no preview until downloaded) | todo |
| 3.7 | Network errors: retry, resume where allowed, clear message for unsupported URLs | todo |

Done when: a pasted URL gives two segments chosen on the timeline and exactly those two files; a middle section of a local video is removed and the rest joined.

## Sprint 4: Projects, Smart Paste, full Batch, packaging

Release: v1.0.0. Status: `todo`. Detailed plan to be written after sprint 3.

| # | Work | Status |
|---|------|--------|
| 4.1 | Project Save & Resume (F05): sources, segments, export settings; save at any step; references files, does not copy video | todo |
| 4.2 | Smart Paste (F06): URLs with timestamps and `full`, grouped per URL, user reviews parsed jobs before running | todo |
| 4.3 | Batch Processing (S01) complete: many sources, per-source settings, Process All | todo |
| 4.4 | Bundle FFmpeg and yt-dlp in installers after a licence check; yt-dlp updatable on its own | todo |
| 4.5 | Cache management and output naming templates | todo |
| 4.6 | Manual full-flow test on a clean machine for each OS | todo |
| 4.7 | First-run notice about platform terms and the Gatekeeper/SmartScreen warning; user guide | todo |

Done when: from a clean install on each OS, several URLs with timestamps are pasted, processed as a batch, the project is saved, reopened and continued.

---

## Decision log (short)

| Date | Decision |
|------|----------|
| 2026-11-23 | Stack final: Rust + GPUI + GPUI Kit; no WebView, no Tauri, no React |
| 2026-11-23 | Auto-updater written in Rust; repo is public; no code-signing certificate |
| 2026-11-23 | Release and update first (sprint 1); dev and production builds separate |
| 2026-11-23 | UI in English and Vietnamese, language modal at first launch, changeable in Settings |
| 2026-11-23 | README and GitHub About in English only |
| 2026-11-23 | At least 85% of the code is Rust; helper scripts are Rust (`cargo xtask`) |
| 2026-11-23 | Real video playback is required; a C or C++ library may be linked for the player |
| 2026-11-23 | Every modal and dialog dims the background and blocks background scroll |
