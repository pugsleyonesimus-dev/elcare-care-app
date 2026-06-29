/**
 * Wallet persistence layer — store and restore provider selection
 */

const STORAGE_KEY = 'elcare.wallet.provider';
const EXPIRY_KEY = 'elcare.wallet.expiry';
const EXPIRY_TTL = 24 * 60 * 60 * 1000; // 24 hours

export type WalletProvider = 'freighter' | 'lobstr' | 'magic';

export function saveWalletProvider(provider: WalletProvider): void {
  if (typeof window === 'undefined') return;
  localStorage.setItem(STORAGE_KEY, provider);
  localStorage.setItem(EXPIRY_KEY, String(Date.now() + EXPIRY_TTL));
}

export function loadWalletProvider(): WalletProvider | null {
  if (typeof window === 'undefined') return null;
  const provider = localStorage.getItem(STORAGE_KEY) as WalletProvider | null;
  const expiry = localStorage.getItem(EXPIRY_KEY);
  
  if (!provider || !expiry || Date.now() > parseInt(expiry, 10)) {
    clearWalletProvider();
    return null;
  }
  
  return provider;
}

export function clearWalletProvider(): void {
  if (typeof window === 'undefined') return;
  localStorage.removeItem(STORAGE_KEY);
  localStorage.removeItem(EXPIRY_KEY);
}
