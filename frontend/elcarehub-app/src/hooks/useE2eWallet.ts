"use client";

import { useCallback, useEffect, useState } from "react";
import { config } from "@/lib/config";
import type { WalletState, WalletStatus } from "./useWallet";

const KEY_STORAGE = "e2e_wallet_public_key";
const PASSPHRASE_STORAGE = "e2e_network_passphrase";
const INSTALLED_STORAGE = "e2e_freighter_installed";

function readStoredPublicKey(): string | null {
  if (typeof window === "undefined") return null;
  return sessionStorage.getItem(KEY_STORAGE);
}

function readStoredPassphrase(): string | null {
  if (typeof window === "undefined") return null;
  return sessionStorage.getItem(PASSPHRASE_STORAGE);
}

/** Playwright-driven wallet for E2E — avoids the Freighter browser extension. */
export function useE2eWallet(): WalletState {
  const [publicKey, setPublicKey] = useState<string | null>(null);
  const [balance, setBalance] = useState<string | null>("100.0000000"); // Mock E2E balance
  const [isLoadingBalance, setIsLoadingBalance] = useState(false);
  const [networkPassphrase, setNetworkPassphrase] = useState<string | null>(null);
  const [isConnecting, setIsConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const syncFromStorage = useCallback(() => {
    setPublicKey(readStoredPublicKey());
    setNetworkPassphrase(readStoredPassphrase() ?? config.networkPassphrase);
  }, []);

  useEffect(() => {
    syncFromStorage();
  }, [syncFromStorage]);

  const isInstalled =
    typeof window === "undefined" ||
    sessionStorage.getItem(INSTALLED_STORAGE) !== "false";

  const isWrongNetwork = !!publicKey &&
    !!networkPassphrase &&
    networkPassphrase !== config.networkPassphrase;

  const status: WalletStatus = !isInstalled
    ? "NOT_INSTALLED"
    : isConnecting
      ? "CONNECTING"
      : !publicKey
        ? "DISCONNECTED"
        : isWrongNetwork
          ? "WRONG_NETWORK"
          : "CONNECTED";

  const connect = useCallback(async () => {
    setIsConnecting(true);
    setError(null);
    try {
      syncFromStorage();
      if (!readStoredPublicKey()) {
        throw new Error("E2E wallet public key not set in sessionStorage.");
      }
    } catch (err: unknown) {
      setError(err instanceof Error ? err.message : "E2E wallet connect failed");
    } finally {
      setIsConnecting(false);
    }
  }, [syncFromStorage]);

  const disconnect = useCallback(() => {
    sessionStorage.removeItem(KEY_STORAGE);
    setPublicKey(null);
    setError(null);
  }, []);

  const refresh = useCallback(async () => {
    syncFromStorage();
  }, [syncFromStorage]);

  return {
    publicKey,
    balance,
    isLoadingBalance,
    networkPassphrase,
    status,
    isInstalled,
    isConnecting,
    isConnected: !!publicKey && !isWrongNetwork,
    isWrongNetwork,
    error,
    connect,
    disconnect,
    refresh,
  };
}
