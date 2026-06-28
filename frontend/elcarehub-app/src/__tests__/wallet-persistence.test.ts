/**
 * Tests for wallet-persistence.ts
 */
import {
  saveWalletProvider,
  loadWalletProvider,
  clearWalletProvider,
  WalletProvider,
} from '@/lib/wallet-persistence';

describe('wallet-persistence', () => {
  beforeEach(() => {
    localStorage.clear();
    jest.clearAllMocks();
  });

  describe('saveWalletProvider', () => {
    it('saves provider and expiry to localStorage', () => {
      saveWalletProvider('freighter');
      expect(localStorage.getItem('elcare.wallet.provider')).toBe('freighter');
      expect(localStorage.getItem('elcare.wallet.expiry')).toBeTruthy();
    });

    it('accepts all valid providers', () => {
      const providers: WalletProvider[] = ['freighter', 'lobstr', 'magic'];
      providers.forEach((provider) => {
        localStorage.clear();
        saveWalletProvider(provider);
        expect(localStorage.getItem('elcare.wallet.provider')).toBe(provider);
      });
    });
  });

  describe('loadWalletProvider', () => {
    it('returns null when nothing saved', () => {
      expect(loadWalletProvider()).toBeNull();
    });

    it('returns saved provider within expiry', () => {
      saveWalletProvider('magic');
      expect(loadWalletProvider()).toBe('magic');
    });

    it('returns null and clears when expired', () => {
      saveWalletProvider('lobstr');
      // Manually set expiry to past
      localStorage.setItem('elcare.wallet.expiry', String(Date.now() - 1000));
      expect(loadWalletProvider()).toBeNull();
      expect(localStorage.getItem('elcare.wallet.provider')).toBeNull();
    });

    it('returns null and clears if expiry missing', () => {
      localStorage.setItem('elcare.wallet.provider', 'freighter');
      localStorage.removeItem('elcare.wallet.expiry');
      expect(loadWalletProvider()).toBeNull();
      expect(localStorage.getItem('elcare.wallet.provider')).toBeNull();
    });
  });

  describe('clearWalletProvider', () => {
    it('removes provider and expiry from localStorage', () => {
      saveWalletProvider('freighter');
      clearWalletProvider();
      expect(localStorage.getItem('elcare.wallet.provider')).toBeNull();
      expect(localStorage.getItem('elcare.wallet.expiry')).toBeNull();
    });
  });
});
