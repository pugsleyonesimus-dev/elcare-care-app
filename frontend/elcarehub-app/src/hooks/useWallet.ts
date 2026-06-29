// ─────────────────────────────────────────────────────────────
// hooks/useWallet.ts — Freighter wallet connection state
// ─────────────────────────────────────────────────────────────

"use client";

import { isE2eMockChain } from "@/lib/e2e-chain-mock";
import { useE2eWallet } from "./useE2eWallet";
import { useFreighterWallet } from "./useFreighterWallet";

export type WalletStatus =
  | "NOT_INSTALLED"
  | "DISCONNECTED"
  | "CONNECTING"
  | "CONNECTED"
  | "WRONG_NETWORK";

export interface WalletState {
  publicKey: string | null;
  balance: string | null;
  isLoadingBalance: boolean;
  networkPassphrase: string | null;
  status: WalletStatus;
  isInstalled: boolean;
  isConnecting: boolean;
  isConnected: boolean;
  isWrongNetwork: boolean;
  error: string | null;
  connect: () => Promise<void>;
  disconnect: () => void;
  refresh: () => Promise<void>;
}

export function useWallet(): WalletState {
  const e2eWallet = useE2eWallet();
  const freighterWallet = useFreighterWallet();
  return isE2eMockChain() ? e2eWallet : freighterWallet;
}
