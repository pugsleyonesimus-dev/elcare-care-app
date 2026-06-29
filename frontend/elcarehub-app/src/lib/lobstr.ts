// -------------------------------------------------------------
// lib/lobstr.ts - Lobstr browser wallet helpers
// Uses @lobstrco/signer-extension-api v2
// Dynamic imports used to prevent SSR crashes in Next.js
// -------------------------------------------------------------

export interface LobstrAccount {
  publicKey: string;
}

async function getLobstrApi() {
  return import("@lobstrco/signer-extension-api");
}

/**
 * Returns true if the Lobstr Signer extension is installed.
 */
export async function isLobstrInstalled(): Promise<boolean> {
  if (typeof window === "undefined") return false;
  try {
    const { isConnected } = await getLobstrApi();
    return await isConnected();
  } catch {
    return false;
  }
}

/**
 * Requests access to Lobstr and returns the public key.
 * v2 API: just call getPublicKey() directly - the extension handles permission prompts.
 */
export async function connectLobstr(): Promise<LobstrAccount> {
  try {
    const installed = await isLobstrInstalled();
    if (!installed) {
      throw new Error("Lobstr extension not found. Please install the Lobstr Signer extension from the Chrome Web Store.");
    }
    const { getPublicKey } = await getLobstrApi();
    const publicKey = await getPublicKey();
    if (!publicKey || typeof publicKey !== "string") {
      throw new Error("Failed to get public key from Lobstr. Ensure your wallet is unlocked.");
    }
    return { publicKey };
  } catch (err: unknown) {
    if (err instanceof Error) throw err;
    throw new Error("An unexpected error occurred while connecting to Lobstr.");
  }
}

/**
 * Asks Lobstr to sign a transaction XDR string.
 * v2 API: signTransaction only takes the XDR, no options object.
 */
export async function signWithLobstr(txXdr: string): Promise<string> {
  const { signTransaction } = await getLobstrApi();
  const result = await signTransaction(txXdr);
  if (!result || typeof result !== "string") {
    throw new Error("Failed to sign transaction with Lobstr.");
  }
  return result;
}

/**
 * Returns the currently connected Lobstr public key, or null if not connected.
 */
export async function getLobstrPublicKey(): Promise<string | null> {
  try {
    const installed = await isLobstrInstalled();
    if (!installed) return null;
    const { getPublicKey } = await getLobstrApi();
    const key = await getPublicKey();
    return typeof key === "string" ? key : null;
  } catch {
    return null;
  }
}