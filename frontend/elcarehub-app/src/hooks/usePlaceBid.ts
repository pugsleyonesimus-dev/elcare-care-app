// ─────────────────────────────────────────────────────────────
// hooks/usePlaceBid.ts — Place bid hook
// ─────────────────────────────────────────────────────────────

"use client";

import { useCallback } from "react";
import { placeBid } from "@/lib/contract";
import { useTxToast } from "./useTxToast";

export function usePlaceBid(bidderPublicKey: string | null) {
  const { run, isRunning: isBidding } = useTxToast();

  const bid = useCallback(
    async (auctionId: number, amountXlm: number): Promise<boolean> => {
      if (!bidderPublicKey) return false;
      const result = await run(
        () => placeBid(bidderPublicKey, auctionId, amountXlm),
        { action: "Bid" }
      );
      return result !== null;
    },
    [bidderPublicKey, run],
  );

  return { bid, isBidding, error: null };
}
