# Contributing to ElcareHub

Thank you for contributing. This guide covers development workflow, testing expectations, and quality gates enforced in CI.

## Development workflow

1. Fork the repository and create a feature branch: `git checkout -b feat/your-feature`
2. Install dependencies in the area you are changing (`frontend/elcarehub-app`, `indexer`, or root for contracts)
3. Run the relevant lint, test, and build commands locally before opening a PR
4. Open a pull request with a clear summary and test plan

## Frontend test coverage (ISSUE-113)

Jest collects coverage in CI for `frontend/elcarehub-app`. Thresholds are set at the **current baseline** so CI does not break unexpectedly; raise them when you add meaningful tests.

| Scope | Policy |
|-------|--------|
| **Global** | Minimum ~60% statements/lines, ~50% branches, ~55% functions |
| **Critical paths** | Higher floors on checkout, listing cards, marketplace hooks, and contract helpers |
| **Ratchet** | When adding tests in an area, bump that area's threshold in `jest.config.js` |

### Commands

```bash
cd frontend/elcarehub-app
npm run test              # unit/component tests
npm run test:coverage     # coverage + threshold enforcement
npm run test:a11y         # jest-axe component checks
npm run test:e2e          # Playwright (starts dev server in mock-chain mode)
```

Coverage reports are written to `frontend/elcarehub-app/coverage/` and uploaded as a CI artifact on every run.

## End-to-end tests (ISSUE-114)

Playwright specs run against `npm run dev:e2e` (`NEXT_PUBLIC_E2E_MOCK_CHAIN=true`) so wallet and chain calls are deterministic via `e2e-chain-mock.ts` and `useE2eWallet`.

- Prefer stable `data-testid` selectors for flows that cross pages or modals
- Core purchase path: connect mock wallet → browse explore → checkout → success
- E2E HTML reports are published as CI artifacts when tests run in CI

## Indexer integration tests (ISSUE-117)

Unit tests mock Postgres/Redis. Integration tests hit **real** ephemeral services:

```bash
cd indexer
docker compose up -d db redis
npm run test:integration
```

CI runs migrations and `prisma db seed` before the integration suite. Keep integration tests focused on query correctness, migrations, and cache behavior.

## Accessibility (ISSUE-118)

- **Component level:** `jest-axe` in `src/__tests__/a11y/`
- **Page level:** Playwright + `@axe-core/playwright` in `tests/e2e/a11y.spec.ts`

Fix serious/critical violations (labels, roles, focus management, contrast) before merging UI changes. Modals must trap focus while open and restore focus on close (`useModalA11y`).

## Schema changes

See [CONTRIBUTING-SCHEMA-CHANGES.md](./CONTRIBUTING-SCHEMA-CHANGES.md) for Prisma migration requirements.

## Commit messages

Follow [Conventional Commits](https://www.conventionalcommits.org/), e.g. `feat(frontend): add checkout coverage thresholds`.
