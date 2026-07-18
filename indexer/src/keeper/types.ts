/**
 * keeper/types.ts
 *
 * Shared type definitions used across the keeper subsystem.  These mirror the
 * Prisma-generated enums so that the rest of the keeper code never imports
 * directly from @prisma/client, keeping test isolation clean.
 */

// ── Target types ─────────────────────────────────────────────────────────────

export type KeeperTargetType =
  | 'ExpireListing'
  | 'FinalizeAuction'
  | 'ReclaimOffer';

// Maps KeeperTargetType → Soroban contract entry-point name
export const ENTRY_POINT: Record<KeeperTargetType, string> = {
  ExpireListing:   'expire_listing',
  FinalizeAuction: 'finalize_auction',
  ReclaimOffer:    'reclaim_offer',
};

// Maps KeeperTargetType → Prometheus entry_point label (snake_case)
export const ENTRY_POINT_LABEL: Record<KeeperTargetType, string> = {
  ExpireListing:   'expire_listing',
  FinalizeAuction: 'finalize_auction',
  ReclaimOffer:    'reclaim_offer',
};

// ── Action lifecycle ─────────────────────────────────────────────────────────

export type KeeperActionStatus =
  | 'Pending'
  | 'Submitted'
  | 'Succeeded'
  | 'Failed'
  | 'Skipped';

// ── Candidate ────────────────────────────────────────────────────────────────

/**
 * A candidate is a lightweight descriptor of a chain object that the keeper
 * believes is ready for maintenance.  Candidate discovery is always cheap
 * (a DB query + optional simulate); no state is written at this point.
 */
export interface KeeperCandidate {
  targetType: KeeperTargetType;
  /** The on-chain ID (listing_id / auction_id / offer_id). */
  targetId: bigint;
}

// ── Transaction pipeline types ───────────────────────────────────────────────

export type SubmitOutcome =
  | { kind: 'succeeded'; txHash: string; feePaid: bigint }
  | { kind: 'fee_bump_needed'; txHash: string; currentFeeStroops: number }
  | { kind: 'permanent_skip'; reason: string }
  | { kind: 'transient_failure'; error: Error };

// ── Error classification ─────────────────────────────────────────────────────

export type ErrorClass = 'permanent' | 'transient';

// ── Keeper cycle stats (returned from runner, used by status endpoint) ────────

export interface KeeperCycleStats {
  startedAt: Date;
  completedAt: Date | null;
  candidatesDiscovered: number;
  actionsAttempted: number;
  actionsSucceeded: number;
  actionsFailed: number;
  actionsSkipped: number;
  feesSpentStroops: bigint;
  budgetExhausted: boolean;
  dryRun: boolean;
}
