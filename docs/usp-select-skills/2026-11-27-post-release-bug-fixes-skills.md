## CHOOSEN SKILLS FOR THIS TASK
- using-superpowers --- Step 0, workflow order and gates
- select-skills-usp --- record skills and get confirmation before each step
- diff-type-subagents --- delegate investigation / review to subagents of other types, orchestrator reviews
- brainstorming --- Step 1, triage of the 15 customer-reported bugs, scope, spec of the fix round
- systematic-debugging --- B1, find root cause and exact location of each bug before proposing any fix
- web-research --- B2, platform facts: macOS Gatekeeper/notarization, Apple Silicon vs Intel, Windows GUI subsystem, native file dialogs, GPUI audio
- business-analyst --- turn the raw bug report into requirements and acceptance criteria (URL feature gap, validation rules)
- gpui-kit --- toast, dialogs, responsive layout, min/max window size, file/folder picker in the GPUI UI
- gpui-kit-design-guides --- toast levels, responsive rules, error placement
- gpui-video-timeline --- video preview and audio playback bug
- media-pipeline-engineering --- file validation, ffprobe checks, yt-dlp URL flow
- durable-local-jobs --- batch result and error reporting from the job queue
- rust --- fixes in the Rust crates (windows subsystem, icons, dialogs)
- security-and-hardening --- file path validation, safe URL input handling, code signing / notarization
- structure-codebase --- where toast, dialog and validation code belong
- writing-plans --- Step 2, ordered fix list after the sprint 1-2 release
- using-git-worktrees --- Step 3, isolated branch
- subagent-driven-development --- Step 4, execution option
- executing-plans --- Step 4, execution option
- test-driven-development --- Step 4, failing test first for every fix
- verification-before-completion --- Step 5
- requesting-code-review --- Step 6
- finishing-a-development-branch --- Step 7

## STEP 1:
- brainstorming
- systematic-debugging
- web-research
- business-analyst
- diff-type-subagents

## STEP 2:
- writing-plans
- rust
- structure-codebase
- test-driven-development
- media-pipeline-engineering
- security-and-hardening
- gpui-kit
- gpui-kit-design-guides
- gpui-video-timeline
- diff-type-subagents

## STEP 3:
- using-git-worktrees

## STEP 4:
- subagent-driven-development
- diff-type-subagents
- test-driven-development
- rust
- media-pipeline-engineering
- security-and-hardening
- gpui-kit
- structure-codebase
- verification-before-completion

## ADDED TO CHOOSEN SKILLS FOR THIS TASK (found missing at Step 4)
- sqlite --- failure kind column and the safe migration of jobs.db (Task 2.5)
- ffmpeg --- build, probe and verify FFmpeg/FFprobe and encoders (Tasks 1.4 to 1.7)
- rust-async-patterns --- job runner pre-flight check and queue updates (Tasks 2.5 to 2.7)
- dispatching-parallel-agents --- parallel groups of delegated tasks under diff-type-subagents

## STEP 4 (added after review):
- using-git-worktrees
- systematic-debugging
- web-research
- gpui-kit-design-guides
- gpui-video-timeline
- durable-local-jobs
- sqlite
- ffmpeg
- rust-async-patterns
- dispatching-parallel-agents
