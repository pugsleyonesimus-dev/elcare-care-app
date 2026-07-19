/**
 * keeper/error-classifier.ts
 *
 * Classifies errors returned by simulateTransaction / sendTransaction /
 * getTransaction into:
 *
 *   permanent  — contract-level revert that will never succeed on retry.
 *                The KeeperAction should be marked Skipped.
 *
 *   transient  — RPC/network failure, sequence collision, resource exhaustion.
 *                The KeeperAction should be retried (possibly with a fee-bump).
 *
 * Soroban contract errors are encoded as `Error(contract, <code>)` in the
 * diagnostic events / result XDR.  We match against the numeric error codes
 * defined in contracts/soroban-marketplace/src/types.rs.
 */

import type { ErrorClass } from './types.js';

// ── MarketplaceError codes that are permanent rejections ──────────────────────
//
// These correspond to the #[contracterror] enum variants in types.rs.
// When the simulator or on-chain execution returns one of these the action
// should never be resubmitted — the precondition the keeper assumed (e.g.
// "this listing is expired") was wrong.

const PERMANENT_CONTRACT_ERROR_CODES = new Set<number>([
  3,  // ListingNotFound
  4,  // ListingNotActive
  9,  // AuctionNotFound
  10, // AuctionNotActive
  14, // AuctionAlreadyFinalized
  16, // OfferNotFound
  18, // OfferNotPending
  20, // ListingSold
  21, // ListingCancelled
  23, // ContractPaused   — halt; don't spam; let the runner surface this
  27, // ListingExpired   — listed as terminal so reclaim doesn't re-fire
  28, // ListingNotExpired — keeper called expire_listing too early
  29, // AuctionNotEnded  — keeper called finalize_auction too early
  33, // InvalidOfferState
  34, // OfferExpired     — reclaim precondition mismatch
]);

// ── String patterns that identify permanent failures ─────────────────────────

const PERMANENT_PATTERNS: RegExp[] = [
  /ListingNotExpired/i,
  /AuctionNotEnded/i,
  /AuctionAlreadyFinalized/i,
  /ListingNotActive/i,
  /ListingNotFound/i,
  /AuctionNotFound/i,
  /OfferNotFound/i,
  /OfferNotPending/i,
  /InvalidOfferState/i,
  /ContractPaused/i,
  /ListingCancelled/i,
  /ListingSold/i,
  /contract error/i,          // generic Soroban contract revert
  /simulate.*failed.*Error\(/i,
];

// ── String patterns that identify transient failures ─────────────────────────

const TRANSIENT_PATTERNS: RegExp[] = [
  /timeout/i,
  /connection reset/i,
  /ECONNREFUSED/i,
  /ENOTFOUND/i,
  /socket hang up/i,
  /rate.?limit/i,
  /too many requests/i,
  /503/,
  /502/,
  /504/,
  /insufficient resource fee/i, // needs fee-bump
  /tx_bad_seq/i,                // sequence number collision → recover + retry
  /tx_insufficient_fee/i,
  /soroban resource limit/i,
];

/**
 * Extract the numeric Soroban contract error code from a diagnostic string
 * such as "Error(Contract, #28)".  Returns null when no code is present.
 */
export function extractContractErrorCode(message: string): number | null {
  // Soroban encodes contract errors as "Error(Contract, #<code>)"
  const match = message.match(/Error\s*\(\s*Contract\s*,\s*#(\d+)\s*\)/i);
  if (match) return Number(match[1]);

  // Fallback: plain numeric code in diagnostic strings
  const codeMatch = message.match(/contract\s+error\s+code[:\s]+(\d+)/i);
  if (codeMatch) return Number(codeMatch[1]);

  return null;
}

/**
 * Classify an error as 'permanent' or 'transient'.
 *
 * Decision priority:
 *  1. Extract a numeric Soroban contract error code → look up in the permanent set.
 *  2. Match permanent string patterns.
 *  3. Match transient string patterns.
 *  4. Unknown errors default to 'transient' (safe — will be retried, then Failed).
 */
export function classifyError(err: unknown): ErrorClass {
  const message = err instanceof Error
    ? `${err.message}\n${err.stack ?? ''}`
    : String(err);

  // 1. Numeric contract error code
  const code = extractContractErrorCode(message);
  if (code !== null && PERMANENT_CONTRACT_ERROR_CODES.has(code)) {
    return 'permanent';
  }

  // 2. Permanent string patterns
  for (const pattern of PERMANENT_PATTERNS) {
    if (pattern.test(message)) return 'permanent';
  }

  // 3. Transient string patterns
  for (const pattern of TRANSIENT_PATTERNS) {
    if (pattern.test(message)) return 'transient';
  }

  // 4. Default: treat as transient so the retry loop can surface it properly
  return 'transient';
}

/**
 * Returns true when the error signals that the transaction fee was too low and
 * a fee-bump resubmission is the correct recovery.
 */
export function isFeeError(err: unknown): boolean {
  const message = err instanceof Error ? err.message : String(err);
  return (
    /insufficient resource fee/i.test(message) ||
    /tx_insufficient_fee/i.test(message) ||
    /fee.?bump/i.test(message)
  );
}

/**
 * Returns true when the error signals a sequence number collision, requiring
 * the keeper to reload the source account's sequence before rebuilding the tx.
 */
export function isSeqError(err: unknown): boolean {
  const message = err instanceof Error ? err.message : String(err);
  return /tx_bad_seq/i.test(message);
}
