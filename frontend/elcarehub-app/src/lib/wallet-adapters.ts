/**
 * Adapter implementations for each wallet provider
 * Wraps provider-specific logic behind the WalletAdapter interface
 */

import { WalletAdapter } from './wallet-adapter';
import { WalletState } from '@/hooks/useWallet';
import { MagicWalletState } from '@/hooks/useMagicWallet';

/**
 * Adapts Freighter/Lobstr WalletState to WalletAdapter interface
 */
export function createExtensionAdapter(
  state: WalletState,
  signFn?: (tx: string) => Promise<string>
): WalletAdapter {
  return {
    isConnected: state.isConnected,
    isConnecting: state.isConnecting,
    publicKey: state.publicKey,
    networkPassphrase: state.networkPassphrase,
    error: state.error,
    connect: state.connect,
    disconnect: state.disconnect,
    signTransaction: signFn || (async () => {
      throw new Error('signTransaction not configured for this adapter');
    }),
  };
}

/**
 * Adapts Magic wallet to WalletAdapter interface
 */
export function createMagicAdapter(
  state: MagicWalletState,
  loginEmail?: (email: string) => Promise<void>,
  loginPasskey?: () => Promise<void>,
  signFn?: (tx: string) => Promise<string>
): WalletAdapter {
  return {
    isConnected: state.isConnected,
    isConnecting: state.isConnecting,
    publicKey: state.publicAddress,
    networkPassphrase: null, // Magic doesn't expose network
    error: state.error,
    connect: loginPasskey || (async () => {
      throw new Error('Magic connect not configured');
    }),
    disconnect: state.logout,
    signTransaction: signFn || (async () => {
      throw new Error('signTransaction not configured for Magic');
    }),
  };
}
