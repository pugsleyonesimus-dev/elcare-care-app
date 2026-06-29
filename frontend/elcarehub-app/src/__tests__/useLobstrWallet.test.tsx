/**
 * Unit tests for useLobstrWallet.ts.
 */
import React from 'react';
import { render, screen, waitFor, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useLobstrWallet } from '@/hooks/useLobstrWallet';

// ── Mocks ─────────────────────────────────────────────────────────────────────

const mockIsLobstrInstalled = jest.fn();
const mockConnectLobstr = jest.fn();
const mockGetLobstrPublicKey = jest.fn();

jest.mock('@/lib/lobstr', () => ({
  isLobstrInstalled: (...args: unknown[]) => mockIsLobstrInstalled(...args),
  connectLobstr: (...args: unknown[]) => mockConnectLobstr(...args),
  getLobstrPublicKey: (...args: unknown[]) => mockGetLobstrPublicKey(...args),
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

// ── Helpers ───────────────────────────────────────────────────────────────────

function LobstrWalletComp() {
  const { status, publicKey, isConnected, isInstalled, error, connect, disconnect, refresh } =
    useLobstrWallet();
  return (
    <div>
      <span data-testid="status">{status}</span>
      <span data-testid="key">{publicKey ?? 'null'}</span>
      <span data-testid="connected">{String(isConnected)}</span>
      <span data-testid="installed">{String(isInstalled)}</span>
      <span data-testid="error">{error ?? 'none'}</span>
      <button data-testid="connect" onClick={connect}>connect</button>
      <button data-testid="disconnect" onClick={disconnect}>disconnect</button>
      <button data-testid="refresh" onClick={refresh}>refresh</button>
    </div>
  );
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('useLobstrWallet', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    jest.useFakeTimers();
  });
  afterEach(() => {
    jest.useRealTimers();
  });

  it('shows NOT_INSTALLED when Lobstr is absent', async () => {
    mockIsLobstrInstalled.mockResolvedValue(false);
    render(<LobstrWalletComp />);
    await waitFor(() =>
      expect(screen.getByTestId('status').textContent).toBe('NOT_INSTALLED')
    );
    expect(screen.getByTestId('installed').textContent).toBe('false');
  });

  it('auto-detects existing connection on mount', async () => {
    mockIsLobstrInstalled.mockResolvedValue(true);
    mockGetLobstrPublicKey.mockResolvedValue('GEXISTING');
    render(<LobstrWalletComp />);
    await waitFor(() =>
      expect(screen.getByTestId('key').textContent).toBe('GEXISTING')
    );
    expect(screen.getByTestId('status').textContent).toBe('CONNECTED');
  });

  it('shows DISCONNECTED when Lobstr installed but not connected', async () => {
    mockIsLobstrInstalled.mockResolvedValue(true);
    mockGetLobstrPublicKey.mockResolvedValue(null);
    render(<LobstrWalletComp />);
    await waitFor(() =>
      expect(screen.getByTestId('status').textContent).toBe('DISCONNECTED')
    );
    expect(screen.getByTestId('connected').textContent).toBe('false');
  });

  it('connect sets publicKey and shows CONNECTED on success', async () => {
    mockIsLobstrInstalled.mockResolvedValue(true);
    mockGetLobstrPublicKey.mockResolvedValue(null);
    mockConnectLobstr.mockResolvedValue({
      publicKey: 'GLOBSTRKEY',
    });

    const user = userEvent.setup({ advanceTimers: jest.advanceTimersByTime });
    render(<LobstrWalletComp />);
    await waitFor(() =>
      expect(screen.getByTestId('status').textContent).toBe('DISCONNECTED')
    );

    await user.click(screen.getByTestId('connect'));
    await waitFor(() =>
      expect(screen.getByTestId('status').textContent).toBe('CONNECTED')
    );
    expect(screen.getByTestId('key').textContent).toBe('GLOBSTRKEY');
  });

  it('connect sets error when user denies or error occurs', async () => {
    mockIsLobstrInstalled.mockResolvedValue(true);
    mockGetLobstrPublicKey.mockResolvedValue(null);
    mockConnectLobstr.mockRejectedValue(new Error('User denied access'));

    const user = userEvent.setup({ advanceTimers: jest.advanceTimersByTime });
    render(<LobstrWalletComp />);
    await waitFor(() =>
      expect(screen.getByTestId('status').textContent).toBe('DISCONNECTED')
    );

    await user.click(screen.getByTestId('connect'));
    await waitFor(() =>
      expect(screen.getByTestId('error').textContent).toBe('User denied access')
    );
  });

  it('disconnect clears publicKey', async () => {
    mockIsLobstrInstalled.mockResolvedValue(true);
    mockGetLobstrPublicKey.mockResolvedValue('GLOBSTRKEY');

    const user = userEvent.setup({ advanceTimers: jest.advanceTimersByTime });
    render(<LobstrWalletComp />);
    await waitFor(() =>
      expect(screen.getByTestId('key').textContent).toBe('GLOBSTRKEY')
    );

    await user.click(screen.getByTestId('disconnect'));
    await waitFor(() =>
      expect(screen.getByTestId('key').textContent).toBe('null')
    );
    expect(screen.getByTestId('status').textContent).toBe('DISCONNECTED');
  });
});
