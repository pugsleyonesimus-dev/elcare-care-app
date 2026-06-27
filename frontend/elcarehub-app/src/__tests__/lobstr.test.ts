/**
 * Unit tests for lobstr.ts (Signer Extension API v2).
 */
import {
  isLobstrInstalled,
  connectLobstr,
  signWithLobstr,
  getLobstrPublicKey
} from '@/lib/lobstr';

// ── Mocks ─────────────────────────────────────────────────────────────────────

const mockIsConnected = jest.fn();
const mockGetPublicKey = jest.fn();
const mockSignTransaction = jest.fn();

// Mock the dynamic import of @lobstrco/signer-extension-api
jest.mock('@lobstrco/signer-extension-api', () => ({
  isConnected: () => mockIsConnected(),
  getPublicKey: () => mockGetPublicKey(),
  signTransaction: (xdr: string) => mockSignTransaction(xdr),
}), { virtual: true });

describe('lobstr library (v2)', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    // Simulate browser environment
    global.window = {} as any;
  });

  afterEach(() => {
    delete (global as any).window;
  });

  describe('isLobstrInstalled', () => {
    it('returns true if extension is connected', async () => {
      mockIsConnected.mockResolvedValue(true);
      const result = await isLobstrInstalled();
      expect(result).toBe(true);
    });

    it('returns false if extension is not connected', async () => {
      mockIsConnected.mockResolvedValue(false);
      const result = await isLobstrInstalled();
      expect(result).toBe(false);
    });

    it('returns false on error', async () => {
      mockIsConnected.mockRejectedValue(new Error('api fail'));
      const result = await isLobstrInstalled();
      expect(result).toBe(false);
    });
  });

  describe('connectLobstr', () => {
    it('returns account on success', async () => {
      mockGetPublicKey.mockResolvedValue('GLOBSTR123');
      const result = await connectLobstr();
      expect(result).toEqual({ publicKey: 'GLOBSTR123' });
      expect(mockGetPublicKey).toHaveBeenCalled();
    });

    it('throws error on failure', async () => {
      mockGetPublicKey.mockResolvedValue(null);
      await expect(connectLobstr()).rejects.toThrow('Failed to get public key from Lobstr.');
    });
  });

  describe('signWithLobstr', () => {
    it('returns signed XDR on success', async () => {
      mockSignTransaction.mockResolvedValue('SIGNEDXDR');
      const result = await signWithLobstr('UNSIGNEDXDR');
      expect(result).toBe('SIGNEDXDR');
      expect(mockSignTransaction).toHaveBeenCalledWith('UNSIGNEDXDR');
    });

    it('throws error on failure', async () => {
      mockSignTransaction.mockResolvedValue(null);
      await expect(signWithLobstr('XDR')).rejects.toThrow('Failed to sign transaction with Lobstr.');
    });
  });

  describe('getLobstrPublicKey', () => {
    it('returns key when installed and connected', async () => {
      mockIsConnected.mockResolvedValue(true);
      mockGetPublicKey.mockResolvedValue('GKEY');
      const result = await getLobstrPublicKey();
      expect(result).toBe('GKEY');
    });

    it('returns null when not installed', async () => {
      mockIsConnected.mockResolvedValue(false);
      const result = await getLobstrPublicKey();
      expect(result).toBe(null);
    });
  });
});
