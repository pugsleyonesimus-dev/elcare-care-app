# Database Schema Changes Checklist

When adding or modifying the database schema:

## Before Writing Code

- [ ] Create a new feature branch: `git checkout -b feat/describe-schema-change`
- [ ] Read `indexer/.github/migration-safety.md` to understand the migration process

## Writing the Migration

- [ ] Use Prisma schema-first approach: update `indexer/prisma/schema.prisma`
- [ ] Run `npx prisma migrate dev --name <description>` to generate migration file
- [ ] Review the generated `.sql` file for:
  - [ ] Correct syntax (no typos)
  - [ ] Indexes on frequently queried columns
  - [ ] Foreign key constraints where needed
  - [ ] DEFAULT values for new columns
  - [ ] NOT NULL constraints only if appropriate
- [ ] For large tables, add `CONCURRENTLY` to index creation:
  ```sql
  CREATE INDEX CONCURRENTLY idx_name ON table (column);
  ```

## Testing Locally

- [ ] Run `npx prisma migrate dev` to apply the migration
- [ ] Run `npm run test` to verify all tests pass
- [ ] Test the migration rollback:
  ```bash
  npx prisma migrate resolve --rolled-back <migration_name>
  npx prisma migrate deploy  # should recreate the migration
  ```
- [ ] Verify schema matches expectations: `npx prisma introspect`

## Commit & PR

- [ ] Add the migration files to git: `git add indexer/prisma/migrations/*/migration.sql`
- [ ] Include schema changes in commit message:
  ```
  feat: describe schema change
  
  - Added `column_name` to `table_name`
  - Created index on frequently queried field
  - Migration is reversible
  ```
- [ ] Open PR with title prefixed by area: `[indexer] describe schema change`

## Code Review

- [ ] Reviewer verifies the migration is reversible
- [ ] Reviewer checks for performance implications (especially on large tables)
- [ ] CI passes shadow DB test (automatic)

## Deployment (Production)

- [ ] Merge PR after approval
- [ ] Redeploy indexer: `kubectl rollout restart deployment/indexer`
- [ ] Monitor:
  - [ ] Logs for errors: `kubectl logs -f deployment/indexer`
  - [ ] `/readyz` endpoint responds with 200
  - [ ] Database metrics (connections, query latency) in Prometheus

## Rollback (if needed)

Follow the procedure in `indexer/.github/migration-safety.md` under "Production Rollback".

---

**Need help?** Refer to `indexer/.github/migration-safety.md` for detailed procedures and troubleshooting.
