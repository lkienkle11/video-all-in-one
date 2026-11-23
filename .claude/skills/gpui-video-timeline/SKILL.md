---
name: gpui-video-timeline
description: Use when building or changing the video preview, thumbnail timeline, playhead, zoom, seek, or segment handles in the GPUI desktop UI of Video All-in-One.
---

# GPUI Video Timeline

Read `gpui-kit` and `gpui-kit-design-guides` first. This skill only adds the rules specific to the video timeline and preview.

## Requirements

- The timeline is a custom GPUI element; do not fake it with generic components.
- The timeline reads and edits the shared segment model from `media-pipeline-engineering`. It owns no copy of segment data.
- Keep time as an integer unit (for example milliseconds or ticks), never floating-point seconds, for storage and comparison. Convert only for display.
- Separate three coordinate spaces and convert explicitly: media time, timeline pixels (depends on zoom and scroll), and screen pixels.
- Playhead, selection, zoom, and scroll are UI state held in GPUI entities. Segment edits go through the core as commands so undo and persistence work.
- Segment handles: drag to resize, snap to playhead and neighboring segments, enforce minimum length, never allow overlap silently (merge or reject, per spec).
- Thumbnails: generate in the background (FFmpeg), cache on disk, load lazily for the visible range only, and show placeholders. Use a virtualized list or ranged rendering; never create one view per thumbnail for a long video.
- Video preview sits behind a single adapter trait (open, seek, play, pause, position, frame), so the player backend can change without touching the timeline. GPUI has no built-in video player; keep the GPUI version pinned to the one `gpui-kit` locks.
- Seeking must not block the UI thread. Debounce scrubbing and drop stale seek requests.
- Keyboard accessible: set in/out points, nudge by frame or second, jump between segments.

## Verification

- Unit-test coordinate conversions and snapping with edge cases (zero length, max zoom, very long source).
- Use GPUI Kit's headless UI integration tests for handle drag, zoom, and seek.
- Manually check a 70-minute source with hundreds of thumbnails for smooth scrolling.
