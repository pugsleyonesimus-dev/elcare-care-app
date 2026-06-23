// -------------------------------------------------------------
// lib/lobstr.ts — Lobstr browser wallet helpers
// Uses @lobstrco/signer-extension-api (mirrors Freighter's API shape)
// -------------------------------------------------------------

import {
  isConnected,
  getPublicKey,
  signTransaction,
  isAllowed,
  setAllowed,
} from "@lobstrco/signer-extension-api";

export interface LobstrAccount {
  publicKey: string;
}

/**
 * Returns true if the Lobstr Signer extension is installed.
 */
export async function isLobstrInstalled(): Promise<boolean> {
  try {
    const result = await isConnected();
    if (typeof result === "boolean") return result;
    if (result && typeof (result as any).isConnected === "boolean") {
      return (result as any).isConnected;
    }
    return !!result;
  } catch {
    return false;
  }
}

/**
 * Requests access to Lobstr and returns the public key.
 */
export async function connectLobstr(): Promise<LobstrAccount> {
  const allowed = await setAllowed();
  const isAllowedResult =
    typeof allowed === "boolean" ? allowed : (allowed as any)?.isAllowed;

  if (!isAllowedResult) {
    throw new Error("Lobstr access was denied by the user.");
  }

  const result = await getPublicKey();
  const publicKey = typeof result === "string" ? result : (result as any)?.publicKey;

  if (!publicKey) {
    throw new Error("Failed to get public key from Lobstr.");
  }

  return { publicKey };
}

/**
 * Asks Lobstr to sign a transaction XDR string.
 */
export async function signWithLobstr(
  txXdr: string,
  networkPassphrase: string
): Promise<string> {
  const result = await signTransaction(txXdr, { networkPassphrase });
  if (typeof result === "string") return result;
  if (result && (result as any).signedTxXdr) return (result as any).signedTxXdr;
  const error = (result as any)?.error;
  throw new Error(error ?? "Failed to sign transaction with Lobstr.");
}

/**
 * Returns the currently connected Lobstr public key, or null if not connected.
 */
export async function getLobstrPublicKey(): Promise<string | null> {
  try {
    const installed = await isLobstrInstalled();
    if (!installed) return null;
    const result = await getPublicKey();
    if (typeof result === "string") return result;
    if (result && (result as any).publicKey) return (result as any).publicKey;
    return null;
  } catch {
    return null;
  }
}
