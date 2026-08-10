# i18n I2 — Design

## Approach

- Add i18n library (prefer `i18next` + `react-i18next` unless repo constraint says otherwise)
- Namespaces by area: `nav`, `dashboard`, `kanban`, `settings`, `sessions`, `employees`, `errors`, `activity`
- Persist locale in settings store / local preference (restart-safe)
- Default `zh-CN`; provide `en` pack

## Data Flow

```
Settings language select → persist → i18n.changeLanguage
Components use t('key') / Trans
Activity labels: migrate getActivityActionLabel into activity namespace
Rust Chinese errors: frontend map table where stable; remainder tracked in leftover list
```

## Tradeoffs

- I2 full extraction is large; use mechanical extraction + review rather than inventing new copy
- Do after send_input so U1/R1 strings are included once

## Compatibility

- zh-CN must match current Chinese UX as closely as practical
- Dark theme unaffected

## Rollback

- Remove provider + revert `t()` calls incrementally; preference key ignored
