# i18n I2 — Implement

## Depends on

`08-10-p3-reports` archived. **Order change**: i18n before send_input (user 2026-08-10). Expect a follow-up extraction pass after U1 lands.

## Checklist

1. [x] Add i18n deps + bootstrap in app root
2. [x] Settings language switch + persistence
3. [x] Extract user-visible strings into zh-CN / en JSON (I2 breadth)
4. [x] Migrate activity label map into i18n or single-source wrapper
5. [x] Error/toast mapping pass + leftover tracker file if needed
6. [x] format:check + test:ci + smoke both locales on main pages

## Validation

```bash
npm run format:check
npm run test:ci
npm run build
```

## Rollback

Revert i18n commit(s); product features from prior children remain.
