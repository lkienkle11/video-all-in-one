---
name: media-pipeline-engineering
description: Use when implementing or modifying video source analysis, cutting, removing sections, online segment downloads, merging, exporting, or FFmpeg/yt-dlp integration in Video All-in-One.
---

# Media Pipeline Engineering

Use the official FFmpeg, FFprobe and yt-dlp documentation as the technical source of truth. Check them (web-research or context7) instead of recalling flags from memory; behavior differs between versions (for example, `--download-sections` has had bugs with some FFmpeg 8.x builds and throttling on long YouTube videos).

## Core requirements

- Keep media operation planning (building an operation plan) separate from process execution (running FFmpeg/yt-dlp).
- Normalize all time intervals before processing: sort, clamp to source duration, merge overlapping and adjacent ranges.
- Reuse the same segment model for keep, remove, and export.
- Support fast stream-copy cuts and precise re-encoded cuts as two explicit modes.
- Never promise frame-accurate output from stream copy; cuts snap to keyframes.
- For online segments, evaluate partial download and always provide a full-source fallback.
- Do not download the same source twice within one batch when a valid cached copy exists.
- Validate codec, container, audio streams, and timestamps before concatenation.
- Write to temporary output files and finalize (rename) only after successful validation.
- Never silently overwrite or delete the user's source video.
- Cache rules: key by source identity plus format; never delete a cache entry that a running job uses.

## Verification

Use deterministic media fixtures covering:

- Single and multiple segments
- Overlapping and adjacent intervals
- Removed sections at start, middle, and end
- Keyframe and non-keyframe boundaries
- Audio/video stream mismatches
- Separate and merged exports
- Corrupt or unavailable source media

Verify output with FFprobe (duration, streams, codecs) and, where needed, an FFmpeg decode check (`-f null -`). Property tests (proptest) are a good fit for interval normalization.
