/**
 * keeper/idempotency.ts
 *
 * Wraps all KeeperAction DB operations and enforces the state-machine rules:
 *
 *   Pending   → Submitted (on sendTransaction)
 *   Submitted → Succeeded (on getTransaction SUCCESS)
 *   Submitted → Failed    (on exhausted retries / permanent on-chain fail → see note)
 *   Pending   → Skipped   (on permanent simulation revert)
 *   *         → Skipped   (on permanent on-chain revert; overrides Failed)
 *
 * The unique index on (targetType, targetId) in the DB ensures that a second
 * invocation for the same target never creates a duplicate row.  If a row
 * already exists in Submitted state, the runner skips that candidate —
 * preventing double-submission after a restart.
 *
 * Rows in Failed state are eligible for a new cycle attempt ONLY when
 * attempts < max_attempts (enforced by the runner, not here).
 * Rows in Succeeded or Skipped are permanent terminal states and are never
 * re-queued.
 */

import prisma from '../db.js';
import type { KeeperCandidate, KeeperActionStatus } from './types.js';

// Re-export so callers never import from @prisma/client directly.
export type { KeeperActionStatus };

// ── Queries ───────────────────────────────────────────────────────────────────

/**
 * Return the existing KeeperAction for this candidate, or null.
 * Used by the runner to decide whether to skip, retry, or create.
 */
export async function findAction(candidate: KeeperCandidate) {
  return prisma.keeperAction.findUnique({
    where: {
      targetType_targetId: {
        targetType: candidate.targetType,
        targetId: candidate.targetId,
      },
    },
  });
}

/**
 * Return all actions in a given status (used by the status API endpoint
 * and the runner's resume-after-restart logic).
 */
export async function findActionsByStatus(status: KeeperActionStatus) {
  return prisma.keeperAction.findMany({
    where: { status },
    orderBy: { updatedAt: 'asc' },
  });
}

// ── State transitions ─────────────────────────────────────────────────────────

/**
 * Create a new KeeperAction in Pending state.
 * Uses upsert so a concurrent runner or restart doesn't cause a unique-key
 * violation; if a row already exists it is left untouched.
 */
export async function createOrSkipAction(candidate: KeeperCandidate) {
  return prisma.keeperAction.upsert({
    where: {
      targetType_targetId: {
        targetType: candidate.targetType,
        targetId: candidate.targetId,
      },
    },
    create: {
      targetType: candidate.targetType,
      targetId:   candidate.targetId,
      status:     'Pending',
      attempts:   0,
    },
    update: {}, // do not overwrite an existing row
  });
}

/**
 * Transition Pending → Submitted.  Records the txHash and increments attempts.
 */
export async function markSubmitted(id: number, txHash: string) {
  return prisma.keeperAction.update({
    where: { id },
    data: {
      status:  'Submitted',
      txHash,
      attempts: { increment: 1 },
      lastError: null,
    },
  });
}

/**
 * Transition Submitted → Succeeded.  Records the fee paid.
 */
export async function markSucceeded(id: number, txHash: string, feePaid: bigint) {
  return prisma.keeperAction.update({
    where: { id },
    data: {
      status:  'Succeeded',
      txHash,
      feePaid,
      lastError: null,
    },
  });
}

/**
 * Transition → Failed.  Preserves the last error for operator inspection.
 * Does not reset attempts so the runner can enforce a max-attempts cap.
 */
export async function markFailed(id: number, error: string) {
  return prisma.keeperAction.update({
    where: { id },
    data: {
      status:    'Failed',
      lastError: error.slice(0, 4096), // cap to avoid oversized DB values
    },
  });
}

/**
 * Transition → Skipped (permanent).  A Skipped action is never re-queued.
 */
export async function markSkipped(id: number, reason: string) {
  return prisma.keeperAction.update({
    where: { id },
    data: {
      status:    'Skipped',
      lastError: reason.slice(0, 4096),
    },
  });
}

/**
 * Reset a Failed action back to Pending so it can be retried.
 * Only valid from Failed status; Succeeded/Skipped/Submitted must not be reset.
 */
export async function resetForRetry(id: number) {
  return prisma.keeperAction.update({
    where: { id, status: 'Failed' },
    data: {
      status:    'Pending',
      txHash:    null,
      lastError: null,
    },
  });
}

// ── Summary queries (used by /keeper/status) ──────────────────────────────────

export async function getActionSummary() {
  const counts = await prisma.keeperAction.groupBy({
    by: ['status'],
    _count: { id: true },
  });

  const summary: Record<string, number> = {
    Pending:   0,
    Submitted: 0,
    Succeeded: 0,
    Failed:    0,
    Skipped:   0,
  };

  for (const row of counts) {
    summary[row.status] = row._count.id;
  }

  return summary;
}

export async function getRecentActions(limit = 20) {
  return prisma.keeperAction.findMany({
    orderBy: { updatedAt: 'desc' },
    take: limit,
    select: {
      id:         true,
      targetType: true,
      targetId:   true,
      txHash:     true,
      status:     true,
      attempts:   true,
      lastError:  true,
      feePaid:    true,
      createdAt:  true,
      updatedAt:  true,
    },
  });
}
