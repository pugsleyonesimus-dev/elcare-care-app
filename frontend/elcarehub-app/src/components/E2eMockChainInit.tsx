"use client";

import { useEffect } from "react";
import { isE2eMockChain, registerE2eMockListingsOnWindow } from "@/lib/e2e-chain-mock";

/** Registers browser hooks for Playwright E2E chain mocks on first paint. */
export function E2eMockChainInit() {
  useEffect(() => {
    if (isE2eMockChain()) {
      registerE2eMockListingsOnWindow();
    }
  }, []);

  return null;
}
