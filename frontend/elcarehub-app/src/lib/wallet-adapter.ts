/**
 * Unified wallet adapter interface
 * All wallet providers (Freighter, Lobstr, Magic) implement this interface
 */

export interface WalletAdapter {
  // Connection state
  isConnected: boolean;
  isConnecting: boolean;
  publicKey: string | null;
  networkPassphrase: string | null;

  // Error state
  error: string | null;

  // Operations
  connect(): Promise<void>;
  disconnect(): void;
  signTransaction(tx: string): Promise<string>;
}

export type WalletAdapterType = 'freighter' | 'lobstr' | 'magic';
