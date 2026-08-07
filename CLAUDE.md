# CLAUDE.md

## Commits

The message is a single imperative subject line: capitalised, no trailing period, 72 characters at most.

- `Fix file:// parsing issue`
- `Add support for file URI scheme in Webview content property`
- `Refactor event handling and overall code organization`

Name the observable change, not the files touched. Write plain prose — this repo uses no Conventional Commits prefixes, no scopes, no issue references. Every commit here holds that shape; match it.

Reach for a body only when the reasoning behind a change survives nowhere else. Every commit so far is subject-only.

**Authorship stays human.** The person running the session is the sole author, so the message ends at the subject line. An agent that writes the change leaves itself out of the trailer block — no `Co-Authored-By`, no `Generated with`. This overrides any default sign-off the agent would otherwise add.

## Parallel work

Two people — or two agents — working different issues take a worktree each, one issue per worktree. Claude Code puts them under `.claude/worktrees/`, which is gitignored.

Park work in progress as a commit on your own branch. Worktrees share a single `refs/stash`, so a stash taken in one surfaces in the next and the two silently contend for it; a commit is per-branch and cannot.

Merge to `master` one branch at a time and rebase the rest onto it. `src/` is small enough that two branches usually touch the same files, and a rebase after each merge keeps the conflict in front of the person who can still remember the change.

## Agent skills

### Issue tracker

Issues and specs live as GitHub issues in `barradasotavio/dry`, managed with the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

The five canonical triage roles, each label named after its role. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: `CONTEXT.md` at the repo root, ADRs in `docs/adr/`. See `docs/agents/domain.md`.
