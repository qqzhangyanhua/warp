# Agent Instructions

## Read First
- [docs/project-map.md](docs/project-map.md)
- [docs/testing.md](docs/testing.md)
- [docs/dangerous-areas.md](docs/dangerous-areas.md)
- [docs/debugging.md](docs/debugging.md)
- [CONTEXT.md](CONTEXT.md) — domain vocabulary; use these terms and avoid the listed synonyms.
- [docs/adr/](docs/adr/) — architectural decision records for the area you are about to change.

## Default Workflow
1. Understand the task and current workspace state.
2. Search for relevant files before editing.
3. Make the smallest safe change.
4. Add or update tests when behavior changes.
5. Run [scripts/check.sh](scripts/check.sh) before PR-ready work.
6. Report changed files, validation results, and remaining risks.

## Agent Skills
- Issues are tracked in GitHub Issues for `qqzhangyanhua/warp`; external PRs are not a triage request surface. See [docs/agents/issue-tracker.md](docs/agents/issue-tracker.md).
- Use the canonical triage labels in [docs/agents/triage-labels.md](docs/agents/triage-labels.md).
- Domain docs use the layout described in [docs/agents/domain.md](docs/agents/domain.md).
- For GUI-only work, prefer GUI-specific skills such as `gui-ui-guidelines` and `gui-integration-test`.
- For TUI-only work, prefer TUI-specific skills such as `tui-ui-guidelines`, `tui-testing`, and `tui-verify-change`.

## Do Not
- Edit generated files directly.
- Modify secrets or local environment files.
- Change auth, billing, permissions, migrations, infra, release/deployment, or public APIs without calling out the risk.
- Hide failing or skipped checks.
- Remove unrelated comments or rewrite unrelated code.

## Large or High-Risk Changes
Use [docs/agent-plan-template.md](docs/agent-plan-template.md) before editing. Stop after the plan for high-risk areas and ask for human review.
