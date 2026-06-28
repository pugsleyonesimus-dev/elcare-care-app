// -------------------------------------------------------------
// hooks/useLobstrWallet.ts — Lobstr extension wallet state
// -------------------------------------------------------------

"use client";

import { useState, useCallback, useEffect } from "react";
import { Horizon } from "@stellar/stellar-sdk";
import {
  isLobstrInstalled,
  connectLobstr,
  getLobstrPublicKey,
} from "@/lib/lobstr";
import { config } from "@/lib/config";
import { trackEvent } from "@/providers/PostHogProvider";
import type { WalletState, WalletStatus } from "./useWallet";

export function useLobstrWallet(): WalletState {
  const [publicKey, setPublicKey] = useState<string | null>(null);
  const [balance, setBalance] = useState<string | null>(null);
  const [isLoadingBalance, setIsLoadingBalance] = useState(false);
  const [networkPassphrase, setNetworkPassphrase] = useState<string | null>(null);
  const [isInstalled, setIsInstalled] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchBalance = useCallback(async (key: string) => {
    setIsLoadingBalance(true);
    try {
      const server = new Horizon.Server(config.horizonUrl);
      const account = await server.loadAccount(key);
      const nativeBalance = account.balances.find(
        (b) => b.asset_type === "native",
      );
      setBalance(nativeBalance ? nativeBalance.balance : "0.0000000");
    } catch (err) {
      console.error("Failed to fetch balance:", err);
      setBalance(null);
    } finally {
      setIsLoadingBalance(false);
    }
  }, []);

  // Lobstr extension does not expose network passphrase via getPublicKey,
  // so we treat the network as correct once connected (user manages network
  // in the Lobstr app itself). We mark wrong network if user explicitly set
  // a mismatched passphrase after connect.
  const isWrongNetwork =
    !!publicKey &&
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

  const refresh = useCallback(async () => {
    const installed = await isLobstrInstalled();
    setIsInstalled(installed);
    if (installed) {
      try {
        const key = await getLobstrPublicKey();
        if (key) {
          setPublicKey(key);
          if (key !== publicKey) {
            fetchBalance(key);
          }
        }
      } catch (err) {
        console.error("Lobstr auto-detection error:", err);
      }
    }
  }, [publicKey, fetchBalance]);

  useEffect(() => {
    refresh();
    const interval = setInterval(refresh, 800);
    const timeout = setTimeout(() => clearInterval(interval), 4000);
    return () => {
      clearInterval(interval);
      clearTimeout(timeout);
    };
  }, [refresh]);

  // Periodic balance refresh when connected
  useEffect(() => {
    if (status === "CONNECTED" && publicKey) {
      const interval = setInterval(() => fetchBalance(publicKey), 10000);
      return () => clearInterval(interval);
    }
  }, [status, publicKey, fetchBalance]);

  const connect = useCallback(async () => {
    setIsConnecting(true);
    setError(null);
    try {
      const account = await connectLobstr();
      setPublicKey(account.publicKey);
      // Lobstr does not expose network passphrase; assume configured network
      setNetworkPassphrase(config.networkPassphrase);
      trackEvent.walletConnected("lobstr", account.publicKey);
      fetchBalance(account.publicKey);
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Failed to connect Lobstr";
      setError(msg);
      trackEvent.walletConnectionDropOff("connection_failed", "lobstr");
    } finally {
      setIsConnecting(false);
    }
  }, [fetchBalance]);

  const disconnect = useCallback(() => {
    setPublicKey(null);
    setBalance(null);
    setNetworkPassphrase(null);
    setError(null);
  }, []);

  return {
    publicKey,
    balance,
    isLoadingBalance,
    networkPassphrase,
    status,
    isInstalled,
    isConnecting,
    isConnected: status === "CONNECTED",
    isWrongNetwork,
    error,
    connect,
    disconnect,
    refresh,
  };
}
