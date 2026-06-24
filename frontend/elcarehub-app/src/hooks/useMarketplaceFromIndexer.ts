// ─────────────────────────────────────────────────────────────
// hooks/useMarketplaceFromIndexer.ts — Optimized marketplace hook
// ─────────────────────────────────────────────────────────────

"use client";

import { useState, useEffect, useCallback } from "react";
import { getAllListings, Listing } from "@/lib/contract";
import { fetchListings } from "@/lib/indexer";
import { getReadableErrorMessage } from "@/lib/errors";
import { useTransientErrorToast } from "./useTransientErrorToast";

/**
 * Fetches listings with indexer optimization.
 * Makes 1 API call instead of N contract calls.
 * Falls back to on-chain scan only if the indexer is unreachable.
 */
export function useMarketplaceFromIndexer(opts?: { status?: string; limit?: number; offset?: number }) {
  const [listings, setListings] = useState<Listing[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [total, setTotal] = useState<number | null>(null);
  
  useTransientErrorToast(error);

  const refresh = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      try {
        // Step 1: Try the indexer API (1 RPC/HTTP call for all results)
        const res = await fetchListings({
          status: opts?.status || "Active",
          limit: opts?.limit || 100,
          offset: opts?.offset || 0,
        });
        
        if (res.listings && res.listings.length >= 0) {
          setListings(res.listings as Listing[]);
          setTotal(res.total ?? res.listings.length);
          return;
        }
      } catch (e) {
        console.warn("[indexer] Fallback to on-chain scan:", e);
      }

      // Step 2: Fallback to on-chain scan (N CONTRACT CALLS — backup only)
      const all = await getAllListings();
      
      // Basic filtering to match indexer options if needed
      let filtered = all;
      if (opts?.status) {
        filtered = all.filter(l => l.status === opts.status);
      }
      
      setListings(filtered);
      setTotal(filtered.length);
      
    } catch (err: unknown) {
      setError(getReadableErrorMessage(err, "Failed to load marketplace listings"));
    } finally {
      setIsLoading(false);
    }
  }, [opts?.status, opts?.limit, opts?.offset]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return { listings, total, isLoading, error, refresh };
}
