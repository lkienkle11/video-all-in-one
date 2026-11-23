<!-- begin-rule-commit-->

## Git Commit Rules

- Follow the project's Commitlint configuration and Conventional Commits specification.
- Commit title must use the format: `type(scope): description`.
- Commit body must contain all four sections, in this order:

  **Scope:**
  Describe the affected scope.

  **What did:**
  Summarize the changes made.

  **Reviewed:**
  Summarize what was reviewed.

  **Tested:**
  List tests executed and their results.

- Never include AI attribution or co-author trailers, including `Co-Authored-By: Claude`, `Co-Authorized by: Claude`, or any other AI model name.
- Do not append automatically generated attribution to commit messages.

<!-- end-rule-commit-->

<!-- begin:superpowers-workflow-rule:start -->

### Superpowers Workflow (Mandatory Order and Approval Gates)

When a request involves building or changing anything, follow the Superpowers workflow in exactly this order. Never jump to code because a request is detailed. Never perform an action (creating files, branches, commits, installs, deletions) that the user has not explicitly allowed.

- **Step 0 — `using-superpowers`:** Before any response or action, including clarifying questions, check which skills apply and invoke them. A request to build something means `brainstorming` comes first, not code.
- **Step 1 — `brainstorming`:**
  - The first message contains only two things: one line saying the user may skip the questions and let the agent start, and ONE open question. If the user skips, state the main technology choices in one line and continue through the normal workflow (spec and plan are still written).
  - Each later message ends with a single question. Do not ask what the user has already stated in the request.
  - Cover: what the thing is, why now and what success looks like, goals / non-goals / anti-goals, prior art, current state, details the user cares about, and how it gets made (platform, language, libraries).
  - Offer reconnaissance (codebase, user files, web) and say what will be looked for; the user decides.
  - Play back understanding in about 200-300 words per part, mark guesses as guesses, and ask what is wrong or missing.
  - Offer the visual companion only in its own message, only when seeing would help, and only once unless the user raises it again.
  - Run research per the "Research Before Design" rule when the idea, feature, or integration is new.
  - For a project, write the design to `docs/superpowers/specs/YYYY-MM-DD-<topic>-design.md` (in this repository under `easy_mindmap_knowledge/docs/superpowers/specs/`). Then run the builder check with a fresh subagent and sort its questions into: already answered (put in the spec), minor (decide and mark as the agent's call), and the user's (ask once, most important first).
  - **GATE:** Do not build anything (no product code, scaffolding, dependency installs, project creation) until the user approves the written design.
- **Step 2 — `writing-plans`:** After the spec is approved, announce "I'm using the writing-plans skill to create the implementation plan", save the plan to `docs/superpowers/plans/YYYY-MM-DD-<feature>.md` with the required header (Goal, Architecture, Tech Stack, Spec, Global Constraints, Review Focus), bite-sized tasks with exact files, interfaces, and test-first steps, then self-review it.
  - **GATE:** The user reads the plan and chooses the execution method (subagent-driven or native). Recommend one with a one-sentence reason. Do not start implementation before the user answers. "Go ahead" after approving the spec is not approval of the plan or of the execution method.
- **Step 3 — isolated workspace (`using-git-worktrees`):** Work in an isolated worktree or branch. Never work on main/master without explicit consent. Creating a branch or worktree requires the user's permission.
- **Step 4 — execution (`executing-plans` or `subagent-driven-development`):** Keep a ledger, run `test-driven-development` for every task (a failing test first, watched failing; minimal code; full suite green), follow each task's steps in order, commit as the plan says, and record every deviation as a `Ruling:`. Stop only for destructive or irreversible operations, security-sensitive actions, side effects outside the workspace (merge, push, publish), or a plan too broken to follow.
- **Step 5 — `verification-before-completion`:** Never claim work is done, fixed, or passing until the verification command has been run and its output read.
- **Step 6 — review (`requesting-code-review`):** One fresh reviewer checks the whole branch. Re-grade findings by effect; fix Critical and Important findings with a failing test first; record Minor findings as deferred.
- **Step 7 — `finishing-a-development-branch`:** Decide how to integrate. Merge, push, and publish require the user's permission. The final report lists every ruling made and every deferred minor.

Order summary: using-superpowers -> brainstorming (+ research) -> spec -> [user approves spec] -> writing-plans -> [user approves plan and picks execution method] -> branch/worktree (with permission) -> execution with TDD -> verification -> review -> finishing the branch.

When the user says "start over" (or an equivalent), restart this workflow from Step 0. Do not ask again about things the user already specified, and do not continue previous work that was never approved.

<!-- end:superpowers-workflow-rule:end -->

<!-- begin:release-tag-rule:start -->
## Release Tag Rule (changelog, version info and size-based version number)

When the user asks to create a tag or a release (for example "tag v0.1.2", "merge và tag", "release"):

1. **Do not tag yet.** First assess the size of the work in this release and propose the version number; wait for the user's answer.
2. **Assess the size of task** from evidence, not from feeling: the commits since the last published tag (`git log <last-tag>..HEAD`), the files and lines changed, user-visible features added or changed, database migrations, breaking or incompatible changes, and the number and weight of plan tasks. Rate it as one of: **huge** (siêu big), **big**, **somewhat big** (hơi big), **medium** (vừa), **small** (nhỏ).
3. **Propose the version `X.Y.Z`** from that size. Recommended mapping (the user decides):
   - huge -> **major** (`X+1.0.0`), or a minor jump while the app is still 0.x and the user has not declared 1.0.0
   - big -> **minor** (`X.Y+1.0`)
   - somewhat big -> **minor** when it adds a user-visible feature, otherwise **patch**
   - medium and small -> **patch** (`X.Y.Z+1`)
   A release that only exercises the pipeline (no code change) is always **patch**.
4. **Ask the user** with `AskUserQuestion` (or a direct question when the tool is unavailable): show the size rating with the 2 to 3 pieces of evidence behind it, your recommended `X.Y.Z` with the reason, and the alternatives (the next patch, minor and major numbers). The user may also type another number. **Wait for the answer.** Never choose the number alone, and never reuse a number that is already published.
5. **After the user decides, prepare the release content before the tag**, in the same release commit:
   - Write the `CHANGELOG.md` section for that version in **both languages** (`### English`, then `### Tiếng Việt`), describing what changed for the user (this is the "info version" shown in the update modal, in Settings and on the GitHub Release page). No placeholder text.
   - Set the version in all version files (`node scripts/release/set-version.mjs X.Y.Z`, then `package-lock.json`), as described in `easy_mindmap_knowledge/docs/RELEASING.md`.
   - Commit it as `chore(release): X.Y.Z` with the four body sections required by the commit rules.
6. Only then, with the user's permission for each outward step, merge, push the tag `vX.Y.Z`, watch the release workflow, and verify the published Release and `latest.json` (version, sizes, notes).
7. Report to the user: the final version, the size rating used, what the release notes say, and the verification result.

<!-- end:release-tag-rule:end -->