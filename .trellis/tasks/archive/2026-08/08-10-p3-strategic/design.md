# P3 Strategic — Parent Design

## Role

Parent owns cross-child contracts and integration review. Implementation happens in children in order **O1**: reports → send_input → i18n.

## Cross-Child Contracts

| Concern | Contract |
|---------|----------|
| Capability honesty | Matrix `send_input` flips only when mid-session write path exists; UI uses `can()` fail-closed |
| Locale | New user-visible strings from reports/send_input land as Chinese first; i18n child extracts them last |
| Scope filters | Report aggregations respect `projectId` + `environmentMode` + `selectedSshConfigId` |
| SSH | send_input Local required for true engines; SSH best-effort with disable+reason |
| Out of scope | Issues sync; resume-as-send_input; new `/reports` route |

## Integration Acceptance

1. Archive children independently after each check pass
2. Parent verifies `TASK.md` P3 checkboxes + analysis doc honesty
3. Run `format:check`, clippy `-D warnings`, `npm run test:ci`, relevant cargo tests

## Rollback Shape

- Per-child revert is preferred (atomic commits per child)
- Capability matrix must never advertise false `send_input` mid-rollback
