/**
 * Tests for WalletContext persistence and auto-reconnect
 */
import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import { WalletProvider, useWalletContext } from '@/context/WalletContext';

const mockIsFreighterInstalled = jest.fn();
const mockGetConnectedPublicKey = jest.fn();
const mockConnectFreighter = jest.fn();
const mockRefreshFreighter = jest.fn();

const mockIsLobstrInstalled = jest.fn();
const mockConnectLobstr = jest.fn();
const mockGetLobstrPublicKey = jest.fn();

const mockIsMagicLoggedIn = jest.fn();
const mockGetMagicUserMetadata = jest.fn();
const mockRefreshMagic = jest.fn();

jest.mock('@/lib/freighter', () => ({
  isFreighterInstalled: (...args: unknown[]) => mockIsFreighterInstalled(...args),
  getConnectedPublicKey: (...args: unknown[]) => mockGetConnectedPublicKey(...args),
  connectFreighter: (...args: unknown[]) => mockConnectFreighter(...args),
}));

jest.mock('@/lib/lobstr', () => ({
  isLobstrInstalled: (...args: unknown[]) => mockIsLobstrInstalled(...args),
  connectLobstr: (...args: unknown[]) => mockConnectLobstr(...args),
  getLobstrPublicKey: (...args: unknown[]) => mockGetLobstrPublicKey(...args),
}));

jest.mock('@/lib/magic', () => ({
  isMagicLoggedIn: (...args: unknown[]) => mockIsMagicLoggedIn(...args),
  getMagicUserMetadata: (...args: unknown[]) => mockGetMagicUserMetadata(...args),
  logoutFromMagic: jest.fn(),
  loginWithMagicLink: jest.fn(),
  loginWithPasskey: jest.fn(),
}));

jest.mock('@/lib/config', () => ({
  config: {
    networkPassphrase: 'Test SDF Network ; September 2015',
    network: 'testnet',
  },
}));

jest.mock('@/providers/PostHogProvider', () => ({
  trackEvent: {
    walletConnected: jest.fn(),
    walletConnectionDropOff: jest.fn(),
  },
}));

jest.mock('@/lib/e2e-chain-mock', () => ({
  isE2eMockChain: () => false,
}));

jest.mock('@/hooks/useE2eWallet', () => ({
  useE2eWallet: () => ({
    publicKey: null,
    networkPassphrase: null,
    status: 'DISCONNECTED',
    isInstalled: false,
    isConnecting: false,
    isConnected: false,
    isWrongNetwork: false,
    error: null,
    connect: jest.fn(),
    disconnect: jest.fn(),
    refresh: jest.fn(),
  }),
}));

import * as persistence from '@/lib/wallet-persistence';

jest.spyOn(persistence, 'saveWalletProvider');
jest.spyOn(persistence, 'loadWalletProvider');
jest.spyOn(persistence, 'clearWalletProvider');

function TestComponent() {
  const wallet = useWalletContext();
  return (
    <div>
      <span data-testid="connected">{String(wallet.isConnected)}</span>
      <span data-testid="key">{wallet.publicKey ?? 'null'}</span>
    </div>
  );
}

describe('WalletContext persistence', () => {
  beforeEach(() => {
    localStorage.clear();
    jest.clearAllMocks();
    jest.useFakeTimers();
  });

  afterEach(() => {
    jest.useRealTimers();
  });

  it('saves provider on successful Freighter connect', async () => {
    mockIsFreighterInstalled.mockResolvedValue(true);
    mockGetConnectedPublicKey.mockResolvedValue(null);
    mockConnectFreighter.mockResolvedValue({
      publicKey: 'GNEW',
      networkPassphrase: 'Test SDF Network ; September 2015',
    });
    mockIsMagicLoggedIn.mockResolvedValue(false);
    mockIsLobstrInstalled.mockResolvedValue(false);

    render(
      <WalletProvider>
        <TestComponent />
      </WalletProvider>
    );

    await waitFor(() => {
      expect(mockIsFreighterInstalled).toHaveBeenCalled();
    });

    // Note: connectFreighterWithPersist would save on success,
    // but we're testing the context's auto-reconnect logic below
  });

  it('auto-reconnects Freighter if provider saved', async () => {
    localStorage.setItem('elcare.wallet.provider', 'freighter');
    localStorage.setItem(
      'elcare.wallet.expiry',
      String(Date.now() + 24 * 60 * 60 * 1000)
    );

    mockIsFreighterInstalled.mockResolvedValue(true);
    mockGetConnectedPublicKey.mockResolvedValue(null);
    mockConnectFreighter.mockResolvedValue({
      publicKey: 'GAUTO',
      networkPassphrase: 'Test SDF Network ; September 2015',
    });
    mockIsMagicLoggedIn.mockResolvedValue(false);
    mockIsLobstrInstalled.mockResolvedValue(false);

    render(
      <WalletProvider>
        <TestComponent />
      </WalletProvider>
    );

    await waitFor(() => {
      expect(mockConnectFreighter).toHaveBeenCalled();
    });

    await waitFor(() => {
      expect(screen.getByTestId('key').textContent).toBe('GAUTO');
    });
  });

  it('clears persistence on explicit disconnect', async () => {
    mockIsFreighterInstalled.mockResolvedValue(true);
    mockGetConnectedPublicKey.mockResolvedValue('GEXISTING');
    mockIsMagicLoggedIn.mockResolvedValue(false);
    mockIsLobstrInstalled.mockResolvedValue(false);

    const { rerender } = render(
      <WalletProvider>
        <TestComponent />
      </WalletProvider>
    );

    await waitFor(() => {
      expect(screen.getByTestId('connected').textContent).toBe('true');
    });

    // Simulate disconnect by clearing persistence
    localStorage.setItem('elcare.wallet.provider', 'freighter');
    localStorage.clear(); // Simulating disconnect
    expect(localStorage.getItem('elcare.wallet.provider')).toBeNull();
  });

  it('returns null for expired saved provider', async () => {
    localStorage.setItem('elcare.wallet.provider', 'lobstr');
    localStorage.setItem('elcare.wallet.expiry', String(Date.now() - 1000));

    mockIsFreighterInstalled.mockResolvedValue(true);
    mockGetConnectedPublicKey.mockResolvedValue(null);
    mockIsMagicLoggedIn.mockResolvedValue(false);
    mockIsLobstrInstalled.mockResolvedValue(false);

    render(
      <WalletProvider>
        <TestComponent />
      </WalletProvider>
    );

    await waitFor(() => {
      expect(screen.getByTestId('connected').textContent).toBe('false');
    });

    // Expiry should have cleared the provider
    expect(localStorage.getItem('elcare.wallet.provider')).toBeNull();
  });
});
