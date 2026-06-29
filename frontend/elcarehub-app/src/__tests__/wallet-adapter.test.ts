/**
 * Tests for wallet-adapter unified interface
 */
import { createExtensionAdapter, createMagicAdapter } from '@/lib/wallet-adapters';
import { WalletState } from '@/hooks/useWallet';
import { MagicWalletState } from '@/hooks/useMagicWallet';

describe('wallet-adapter', () => {
  describe('createExtensionAdapter', () => {
    it('exposes WalletState as WalletAdapter interface', () => {
      const mockState: WalletState = {
        publicKey: 'GTEST',
        networkPassphrase: 'Test SDF Network ; September 2015',
        status: 'CONNECTED',
        isInstalled: true,
        isConnecting: false,
        isConnected: true,
        isWrongNetwork: false,
        error: null,
        connect: jest.fn(),
        disconnect: jest.fn(),
        refresh: jest.fn(),
      };

      const adapter = createExtensionAdapter(mockState);

      expect(adapter.isConnected).toBe(true);
      expect(adapter.publicKey).toBe('GTEST');
      expect(adapter.networkPassphrase).toBe('Test SDF Network ; September 2015');
      expect(adapter.error).toBeNull();
    });

    it('delegates connect and disconnect to underlying state', async () => {
      const mockConnect = jest.fn();
      const mockDisconnect = jest.fn();

      const mockState: WalletState = {
        publicKey: null,
        networkPassphrase: null,
        status: 'DISCONNECTED',
        isInstalled: true,
        isConnecting: false,
        isConnected: false,
        isWrongNetwork: false,
        error: null,
        connect: mockConnect,
        disconnect: mockDisconnect,
        refresh: jest.fn(),
      };

      const adapter = createExtensionAdapter(mockState);
      await adapter.connect();
      adapter.disconnect();

      expect(mockConnect).toHaveBeenCalled();
      expect(mockDisconnect).toHaveBeenCalled();
    });

    it('uses provided sign function', async () => {
      const mockSign = jest.fn().mockResolvedValue('signed-tx');
      const mockState: WalletState = {
        publicKey: 'GTEST',
        networkPassphrase: 'Test SDF Network ; September 2015',
        status: 'CONNECTED',
        isInstalled: true,
        isConnecting: false,
        isConnected: true,
        isWrongNetwork: false,
        error: null,
        connect: jest.fn(),
        disconnect: jest.fn(),
        refresh: jest.fn(),
      };

      const adapter = createExtensionAdapter(mockState, mockSign);
      const result = await adapter.signTransaction('tx');

      expect(mockSign).toHaveBeenCalledWith('tx');
      expect(result).toBe('signed-tx');
    });
  });

  describe('createMagicAdapter', () => {
    it('exposes MagicWalletState as WalletAdapter interface', () => {
      const mockState: MagicWalletState = {
        email: 'test@example.com',
        publicAddress: 'GMAGIC',
        status: 'CONNECTED',
        isConnecting: false,
        isConnected: true,
        error: null,
        loginWithEmail: jest.fn(),
        loginWithPasskey: jest.fn(),
        logout: jest.fn(),
        refresh: jest.fn(),
      };

      const adapter = createMagicAdapter(mockState);

      expect(adapter.isConnected).toBe(true);
      expect(adapter.publicKey).toBe('GMAGIC');
      expect(adapter.networkPassphrase).toBeNull();
      expect(adapter.error).toBeNull();
    });

    it('delegates logout to disconnect', async () => {
      const mockLogout = jest.fn();

      const mockState: MagicWalletState = {
        email: 'test@example.com',
        publicAddress: 'GMAGIC',
        status: 'CONNECTED',
        isConnecting: false,
        isConnected: true,
        error: null,
        loginWithEmail: jest.fn(),
        loginWithPasskey: jest.fn(),
        logout: mockLogout,
        refresh: jest.fn(),
      };

      const adapter = createMagicAdapter(mockState);
      await adapter.disconnect();

      expect(mockLogout).toHaveBeenCalled();
    });

    it('uses provided login passkey as connect', async () => {
      const mockLogin = jest.fn();

      const mockState: MagicWalletState = {
        email: null,
        publicAddress: null,
        status: 'DISCONNECTED',
        isConnecting: false,
        isConnected: false,
        error: null,
        loginWithEmail: jest.fn(),
        loginWithPasskey: mockLogin,
        logout: jest.fn(),
        refresh: jest.fn(),
      };

      const adapter = createMagicAdapter(mockState, undefined, mockLogin);
      await adapter.connect();

      expect(mockLogin).toHaveBeenCalled();
    });
  });
});
