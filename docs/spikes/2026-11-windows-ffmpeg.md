# Windows FFmpeg and FFprobe for the installer (spike, task 1.5)

Date: 2026-11-27. Status: research done, nothing wired into the pipeline yet; the decisions at the end belong to the user.

## Question

Customers install nothing. The Windows installer must carry `ffmpeg.exe` and `ffprobe.exe` that are LGPL (no GPL, no non-free) and can encode H.264 for Precise cut. The chain agreed with the user (spec, "Windows FFmpeg has an ordered fallback"): (1) build LGPL FFmpeg with OpenH264 on a Windows runner; (2) a pinned LGPL prebuilt whose licence, source and encoders are verified; (3) stop and ask. This spike reached step 2.

## What was found

Candidate: the LGPL Windows build of the BtbN FFmpeg-Builds project, release branch 8.1, static, `ffmpeg-n8.1.3-41-ga8029b6528-win64-lgpl-8.1.zip` from the release `autobuild-2026-11-26-12-56`.

Verified on 2026-11-27 by the orchestrator:

- The release list and the SHA-256 of each asset come from the GitHub API. The zip was downloaded and its SHA-256 matches: `5eb24166e7f7103dcc12fa6d773674df9491005e78e8069c131e65b3661343da` (170,934,444 bytes).
- The zip holds `bin/ffmpeg.exe`, `bin/ffprobe.exe` (about 134 MB each) and `bin/ffplay.exe`, plus `LICENSE.txt` (GNU LGPL version 3).
- The configuration string embedded in both `ffmpeg.exe` and `ffprobe.exe` contains `--enable-version3`, `--enable-libopenh264`, `--disable-libx264`, `--disable-libx265`, and does **not** contain `--enable-gpl` or `--enable-nonfree`.
- `ffmpeg.exe` also contains the encoder names `h264_mf`, `h264_nvenc`, `h264_qsv` and `h264_amf`.

So this build already links OpenH264. The Windows picker of task 1.6 would choose `libopenh264` (then `h264_mf`), the same as macOS and Ubuntu, and the `h264_mf` fallback is a second choice only.

## What was not verified

- Nothing was run: these are Windows executables and this machine is a Mac. That `ffmpeg.exe -encoders` really lists `libopenh264` and `h264_mf` on a clean Windows machine is for the testers.
- Quality and speed of Precise cut with OpenH264 on Windows.
- Whether `h264_mf` works on Windows "N" editions without the Media Feature Pack (it is only the second choice here).


## The shared variant (checked later the same day)

`ffmpeg-n8.1.3-41-ga8029b6528-win64-lgpl-shared-8.1.zip` from the same release: 81,544,263 bytes, SHA-256 `372016b1248544b4ffc290ae41c9c57e6441bd0da32068ea18032a25b7b36e08` (downloaded; it matches the GitHub API digest).

- `bin/` holds `ffmpeg.exe` (554 KB), `ffprobe.exe` (268 KB), `ffplay.exe` and seven DLLs: `avcodec-62` (94 MB), `avfilter-11` (32 MB), `avformat-62` (20 MB), `swscale-9` (13 MB), `avdevice-62` (5 MB), `avutil-60` (3 MB), `swresample-6` (0.7 MB). Without `ffplay.exe` and the docs, the files to install are about 169 MB, against about 268 MB for the two static executables.
- The configuration string in the DLLs and in both executables is the same as the static build: `--enable-libopenh264`, `--disable-libx264`, `--enable-version3`, `--enable-shared`, and no `--enable-gpl` or `--enable-nonfree`.
- The two executables import only the seven bundled DLLs and Windows system DLLs (the C runtime, `kernel32`, `shell32`). The encoder DLL `avcodec-62.dll` also loads Windows libraries (Media Foundation, Direct3D) at run time; nothing else has to be installed.
- The download is also smaller: about 81 MB against 171 MB.

Packaging consequences: the seven DLLs go in the same `bin` folder as the two executables (not next to the app, so they cannot clash with the FFmpeg inside `libmpv-2.dll`). The packager's licence scan reads the configuration string of each tool; it is present in the executables as well, so the scan works for both variants. The installer must not copy `libmpv-2.dll` into `bin` (see the note at the end).

**Recommendation:** the shared variant, because the installed size is about 100 MB smaller and the download is less than half, with the same licence position and the same encoders.

## Problems and obligations

- **Retention.** BtbN keeps only the daily autobuilds of the last about 14 days (their README, via a search result; not checked on the project's own page). A URL to one cannot be pinned for long. To keep a stable pin the verified zip has to be copied into a release of this repository (an outward action: it needs the user's permission).
- **Size.** The static executables are about 134 MB each, so the installed size is about 270 MB for the two tools. The shared variant is about 169 MB (checked).
- **LGPL duties.** The package must carry the licence text and tell the user where the matching source is: FFmpeg `n8.1.3-41-ga8029b6528` and the build scripts in `BtbN/FFmpeg-Builds`. The packager already copies licence texts and source URLs for the other components; Windows needs the same notice.
- **OpenH264 patents.** The OpenH264 source is BSD licensed, but H.264 patent duties are not decided here: Cisco's patent grant covers its own binary downloads, and this build was compiled by a third party. This stays the user's decision.

## Decisions for the user

1. Approve copying the verified zip (or its shared variant, after it is checked) into a release of this repository as the pinned Windows tools source, with its SHA-256 in `ci.yml` and `release.yml` like the libmpv archives.
2. Static or shared variant. Recommended: shared (about 169 MB installed against about 268 MB; see the section above).
3. The patent point above, before a production release.

## How it plugs into the pipeline (after the decisions)

The Windows packager expects `ffmpeg.exe` and `ffprobe.exe` in the `--libs` folder (task 1.7). The Windows job downloads the pinned archive, checks its SHA-256, and unpacks the two executables (and the DLLs of the shared variant) into the folder that already holds `libmpv-2.dll`. The installer then installs them in `bin`. Task 1.7 notes that the installer also copies every DLL of the libs folder into `bin`; with static tools that duplicates `libmpv-2.dll` and should be narrowed when this layout is fixed.
