// ─────────────────────────────────────────────────────────────
// hooks/useTxToast.ts — Transaction lifecycle toast helper
//
// Wraps an async on-chain action and fires standardised toasts
// for each phase of the Soroban transaction lifecycle:
//
//   submitting → awaiting signature → broadcasting → confirmed
//                                                  ↘ failed
//
// Usage:
//   const { run, isRunning } = useTxToast();
//   const ok = await run(() => buyArtwork(publicKey, listingId), {
//     action: "Purchase",
//   });
// ─────────────────────────────────────────────────────────────

"use client";

import { useCallback, useRef, useState } from "react";
import { useToast } from "@/components/ToastProvider";
import { getReadableErrorMessage, isUserRejectionError } from "@/lib/errors";
import { config } from "@/lib/config";

// ── Types ─────────────────────────────────────────────────────

export type TxLifecyclePhase =
  | "idle"
  | "submitting"
  | "signing"
  | "broadcasting"
  | "confirming"
  | "success"
  | "error";

export interface UseTxToastOptions {
  /**
   * Short human-readable label for the action shown in toast messages,
   * e.g. "Purchase", "Bid", "Listing", "Offer".
   * Defaults to "Transaction".
   */
  action?: string;

  /**
   * Override the success message. Receives the transaction hash (if
   * available) and must return a string.
   */
  successMessage?: (txHash: string | null) => string;

  /**
   * Duration (ms) for the success toast. Defaults to 8 000 ms so the
   * explorer URL stays visible long enough for users to copy it.
   */
  successDurationMs?: number;

  /**
   * Duration (ms) for error toasts. Defaults to 6 000 ms.
   */
  errorDurationMs?: number;
}

export interface UseTxToastResult {
  /** Execute the async callback with lifecycle toasts. Returns true on success. */
  run: <T>(
    fn: () => Promise<T>,
    opts?: UseTxToastOptions
  ) => Promise<T | null>;

  /** True while the transaction is in any non-idle phase. */
  isRunning: boolean;

  /** Current lifecycle phase. */
  phase: TxLifecyclePhase;
}

// ── Explorer URL helper ───────────────────────────────────────

/**
 * Returns the stellar.expert URL for a transaction hash.
 * Falls back gracefully when hash is null/undefined.
 */
export function getTxExplorerUrl(txHash: string | null | undefined): string | null {
  if (!txHash) return null;
  const network = config.network === "mainnet" ? "mainnet" : "testnet";
  return `https://stellar.expert/explorer/${network}/tx/${txHash}`;
}

// ── Heuristic: extract a transaction hash from a raw result ──

/**
 * Many Soroban SDK responses embed a `hash` field.  We try a few
 * common shapes so callers don't need to pass the hash explicitly.
 */
function extractHash(result: unknown): string | null {
  if (!result || typeof result !== "object") return null;
  const r = result as Record<string, unknown>;
  if (typeof r["hash"] === "string") return r["hash"];
  if (typeof r["txHash"] === "string") return r["txHash"];
  if (typeof r["id"] === "string" && r["id"].length === 64) return r["id"];
  return null;
}

// ── Hook ──────────────────────────────────────────────────────

export function useTxToast(): UseTxToastResult {
  const { pushToast } = useToast();
  const [isRunning, setIsRunning] = useState(false);
  const [phase, setPhase] = useState<TxLifecyclePhase>("idle");

  // Keep a ref so that we can cancel stale in-progress toasts if a new
  // call is made before the previous one completes (edge case).
  const abortRef = useRef(false);

  const run = useCallback(
    async <T>(
      fn: () => Promise<T>,
      opts: UseTxToastOptions = {}
    ): Promise<T | null> => {
      const {
        action = "Transaction",
        successMessage,
        successDurationMs = 8_000,
        errorDurationMs = 6_000,
      } = opts;

      abortRef.current = false;
      setIsRunning(true);

      // Phase 1 — submitting (building + simulating the transaction)
      setPhase("submitting");
      pushToast(`${action}: building transaction…`, "info");

      // Phase 2 — awaiting wallet signature
      // We transition immediately before calling fn() so the user sees
      // "awaiting signature" the moment we hand off to the wallet.
      setPhase("signing");
      pushToast(`${action}: awaiting wallet signature…`, "info");

      let result: T;
      try {
        result = await fn();
      } catch (err: unknown) {
        if (abortRef.current) return null;

        setPhase("error");
        setIsRunning(false);

        if (isUserRejectionError(err)) {
          pushToast(`${action} cancelled — you rejected the request.`, "error", errorDurationMs);
        } else {
          const msg = getReadableErrorMessage(
            err,
            `${action} failed. Please try again.`
          );
          pushToast(msg, "error", errorDurationMs);
        }
        return null;
      }

      if (abortRef.current) return null;

      // Phase 3 — broadcasting (tx was signed, now polling)
      setPhase("broadcasting");
      pushToast(`${action}: broadcasting to the network…`, "info");

      // Phase 4 — confirmed
      setPhase("confirming");

      // Build the success message, optionally including the explorer link.
      const txHash = extractHash(result);
      let successMsg: string;
      if (successMessage) {
        successMsg = successMessage(txHash);
      } else {
        const explorerUrl = getTxExplorerUrl(txHash);
        successMsg = explorerUrl
          ? `${action} confirmed! View on explorer: ${explorerUrl}`
          : `${action} confirmed successfully!`;
      }

      setPhase("success");
      setIsRunning(false);
      pushToast(successMsg, "success", successDurationMs);

      return result;
    },
    [pushToast]
  );

  return { run, isRunning, phase };
}
