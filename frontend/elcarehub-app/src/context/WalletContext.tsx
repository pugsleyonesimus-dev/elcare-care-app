"use client";

import { createContext, useContext, ReactNode, useMemo, useCallback, useEffect, useState } from "react";
import { useWallet, WalletState, WalletStatus } from "@/hooks/useWallet";
import { useMagicWallet, MagicWalletState } from "@/hooks/useMagicWallet";
import { useLobstrWallet } from "@/hooks/useLobstrWallet";
import { saveWalletProvider, loadWalletProvider, clearWalletProvider } from "@/lib/wallet-persistence";

export type WalletType = "freighter" | "lobstr" | "magic" | null;

export interface UnifiedWalletState {
  walletType: WalletType;
  publicKey: string | null;
  balance: string | null;
  isLoadingBalance: boolean;
  isConnected: boolean;
  isConnecting: boolean;
  isWrongNetwork: boolean;
  error: string | null;
  status: WalletStatus | "MAGIC_CONNECTED" | "DISCONNECTED";
  networkPassphrase: string | null;
  isInstalled: boolean;
  connect: () => Promise<void>;
  disconnect: () => void;
  refresh: () => Promise<void>;
  freighter: WalletState;
  lobstr: WalletState;
  magic: MagicWalletState;
  // Per-wallet connect helpers for the modal
  connectFreighter: () => Promise<void>;
  connectLobstr: () => Promise<void>;
  connectMagicEmail: (email: string) => Promise<void>;
  connectMagicPasskey: () => Promise<void>;
}

const WalletContext = createContext<UnifiedWalletState | null>(null);

export function WalletProvider({ children }: { children: ReactNode }) {
  const freighter = useWallet();
  const lobstr = useLobstrWallet();
  const magic = useMagicWallet();
  const [initialized, setInitialized] = useState(false);

  // Auto-reconnect on mount
  useEffect(() => {
    const reconnect = async () => {
      const savedProvider = loadWalletProvider();
      if (!savedProvider) {
        setInitialized(true);
        return;
      }

      try {
        if (savedProvider === 'freighter' && freighter.isInstalled) {
          await freighter.connect();
        } else if (savedProvider === 'lobstr' && lobstr.isInstalled) {
          await lobstr.connect();
        } else if (savedProvider === 'magic') {
          await magic.refresh();
        } else {
          clearWalletProvider();
        }
      } catch (err) {
        console.error('Auto-reconnect failed:', err);
        clearWalletProvider();
      } finally {
        setInitialized(true);
      }
    };

    reconnect();
  }, []); // Only on mount

  const walletType: WalletType = freighter.isConnected
    ? "freighter"
    : lobstr.isConnected
    ? "lobstr"
    : magic.isConnected
    ? "magic"
    : null;

  const activeWallet = freighter.isConnected
    ? freighter
    : lobstr.isConnected
    ? lobstr
    : null;

  const publicKey =
    activeWallet?.publicKey ?? magic.publicAddress ?? null;

  const balance = activeWallet?.balance ?? null;
  const isLoadingBalance = activeWallet?.isLoadingBalance ?? false;

  const status: UnifiedWalletState["status"] = freighter.isConnected
    ? freighter.status
    : lobstr.isConnected
    ? lobstr.status
    : magic.isConnected
    ? "MAGIC_CONNECTED"
    : "DISCONNECTED";

  const connect = useCallback(async () => {
    // Default connect tries Freighter first
    await freighter.connect();
  }, [freighter]);

  const disconnect = useCallback(() => {
    freighter.disconnect();
    lobstr.disconnect();
    // Magic logout is async; fire and forget
    if (magic.isConnected) magic.logout().catch(console.error);
    clearWalletProvider();
  }, [freighter, lobstr, magic]);

  const refresh = useCallback(async () => {
    await Promise.all([freighter.refresh(), lobstr.refresh()]);
  }, [freighter, lobstr]);

  // Wrapper to persist provider choice
  const connectFreighterWithPersist = useCallback(async () => {
    await freighter.connect();
    if (freighter.isConnected) saveWalletProvider('freighter');
  }, [freighter]);

  const connectLobstrWithPersist = useCallback(async () => {
    await lobstr.connect();
    if (lobstr.isConnected) saveWalletProvider('lobstr');
  }, [lobstr]);

  // Magic connect wrappers
  const connectMagicEmail = useCallback(async (email: string) => {
    await magic.loginWithEmail(email);
    if (magic.isConnected) saveWalletProvider('magic');
  }, [magic]);

  const connectMagicPasskey = useCallback(async () => {
    await magic.loginWithPasskey();
    if (magic.isConnected) saveWalletProvider('magic');
  }, [magic]);

  const value = useMemo(
    () => ({
      walletType,
      publicKey,
      balance,
      isLoadingBalance,
      isConnected: freighter.isConnected || lobstr.isConnected || magic.isConnected,
      isConnecting:
        freighter.isConnecting || lobstr.isConnecting || magic.isConnecting,
      isWrongNetwork: activeWallet?.isWrongNetwork ?? false,
      networkPassphrase: activeWallet?.networkPassphrase ?? null,
      isInstalled: freighter.isInstalled || lobstr.isInstalled,
      error: freighter.error ?? lobstr.error ?? magic.error,
      status,
      connect,
      disconnect,
      refresh,
      freighter,
      lobstr,
      magic,
      connectFreighter: connectFreighterWithPersist,
      connectLobstr: connectLobstrWithPersist,
      connectMagicEmail,
      connectMagicPasskey,
    }),
    [
      walletType,
      publicKey,
      balance,
      isLoadingBalance,
      status,
      activeWallet,
      freighter,
      lobstr,
      magic,
      connect,
      disconnect,
      refresh,
      connectFreighterWithPersist,
      connectLobstrWithPersist,
      connectMagicEmail,
      connectMagicPasskey,
    ]
  );

  return (
    <WalletContext.Provider value={value}>{children}</WalletContext.Provider>
  );
}

export function useWalletContext(): UnifiedWalletState {
  const ctx = useContext(WalletContext);
  if (!ctx) {
    throw new Error("useWalletContext must be used inside <WalletProvider>");
  }
  return ctx;
}
