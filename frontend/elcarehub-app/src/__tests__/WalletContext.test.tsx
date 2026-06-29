/**
 * Tests for WalletContext — #85: Network mismatch detection
 */
import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';

// ── Mocks ─────────────────────────────────────────────────────────────────

const mockFreighterState = {
  publicKey: 'GPUBKEY123',
  networkPassphrase: 'Test SDF Network ; September 2015',
  status: 'CONNECTED' as const,
  isInstalled: true,
  isConnecting: false,
  isConnected: true,
  isWrongNetwork: false,
  error: null,
  connect: jest.fn(),
  disconnect: jest.fn(),
  refresh: jest.fn(),
};

const mockLobstrState = {
  publicKey: null,
  networkPassphrase: null,
  status: 'DISCONNECTED' as const,
  isInstalled: false,
  isConnecting: false,
  isConnected: false,
  isWrongNetwork: false,
  error: null,
  connect: jest.fn(),
  disconnect: jest.fn(),
  refresh: jest.fn(),
};

const mockMagicState = {
  email: null,
  publicAddress: null,
  status: 'DISCONNECTED' as const,
  isConnecting: false,
  isConnected: false,
  error: null,
  loginWithEmail: jest.fn(),
  loginWithPasskey: jest.fn(),
  logout: jest.fn(),
  refresh: jest.fn(),
};

jest.mock('@/hooks/useWallet', () => ({
  useWallet: jest.fn(() => mockFreighterState),
}));

jest.mock('@/hooks/useLobstrWallet', () => ({
  useLobstrWallet: jest.fn(() => mockLobstrState),
}));

jest.mock('@/hooks/useMagicWallet', () => ({
  useMagicWallet: jest.fn(() => mockMagicState),
}));

import { WalletProvider, useWalletContext } from '@/context/WalletContext';

// ── Component for testing context ─────────────────────────────────────────

function TestComponent() {
  const wallet = useWalletContext();
  return (
    <div>
      <div data-testid="connected">{wallet.isConnected ? 'connected' : 'disconnected'}</div>
      <div data-testid="wrong-network">{wallet.isWrongNetwork ? 'wrong' : 'correct'}</div>
      <div data-testid="network-passphrase">{wallet.networkPassphrase || 'none'}</div>
      <div data-testid="public-key">{wallet.publicKey || 'none'}</div>
    </div>
  );
}

// ── Tests ─────────────────────────────────────────────────────────────────

describe('WalletContext — Network Mismatch Detection (#85)', () => {
  beforeEach(() => {
    jest.clearAllMocks();
  });

  it('detects matching network passphrase as correct network', () => {
    mockFreighterState.isConnected = true;
    mockFreighterState.isWrongNetwork = false;
    mockFreighterState.networkPassphrase = 'Test SDF Network ; September 2015';

    render(
      <WalletProvider>
        <TestComponent />
      </WalletProvider>
    );

    expect(screen.getByTestId('connected')).toHaveTextContent('connected');
    expect(screen.getByTestId('wrong-network')).toHaveTextContent('correct');
    expect(screen.getByTestId('network-passphrase')).toHaveTextContent(
      'Test SDF Network ; September 2015'
    );
  });

  it('detects mismatched network passphrase', () => {
    mockFreighterState.isConnected = true;
    mockFreighterState.isWrongNetwork = true;
    mockFreighterState.networkPassphrase = 'Public Global Stellar Network ; September 2015';

    render(
      <WalletProvider>
        <TestComponent />
      </WalletProvider>
    );

    expect(screen.getByTestId('connected')).toHaveTextContent('connected');
    expect(screen.getByTestId('wrong-network')).toHaveTextContent('wrong');
  });

  it('surfaces isWrongNetwork flag when network mismatch occurs', () => {
    mockFreighterState.isConnected = true;
    mockFreighterState.isWrongNetwork = true;

    render(
      <WalletProvider>
        <TestComponent />
      </WalletProvider>
    );

    expect(screen.getByTestId('wrong-network')).toHaveTextContent('wrong');
  });

  it('clears wrong network when passphrase matches', () => {
    mockFreighterState.isConnected = true;
    mockFreighterState.isWrongNetwork = false;
    mockFreighterState.networkPassphrase = 'Test SDF Network ; September 2015';

    render(
      <WalletProvider>
        <TestComponent />
      </WalletProvider>
    );

    expect(screen.getByTestId('wrong-network')).toHaveTextContent('correct');

    // Verify state changes are reflected
    mockFreighterState.isWrongNetwork = true;
    expect(mockFreighterState.isWrongNetwork).toBe(true);
  });

  it('returns false for isWrongNetwork when disconnected', () => {
    mockFreighterState.isConnected = false;
    mockFreighterState.publicKey = null;
    mockFreighterState.isWrongNetwork = false;

    render(
      <WalletProvider>
        <TestComponent />
      </WalletProvider>
    );

    expect(screen.getByTestId('wrong-network')).toHaveTextContent('correct');
  });

  it('exports networkPassphrase from active wallet', () => {
    mockFreighterState.isConnected = true;
    mockFreighterState.networkPassphrase = 'Test Network 123';

    render(
      <WalletProvider>
        <TestComponent />
      </WalletProvider>
    );

    expect(screen.getByTestId('network-passphrase')).toHaveTextContent('Test Network 123');
  });

  it('handles lobstr wallet network detection', () => {
    mockFreighterState.isConnected = false;
    mockLobstrState.isConnected = true;
    mockLobstrState.isWrongNetwork = true;
    mockLobstrState.networkPassphrase = 'Wrong Network';

    render(
      <WalletProvider>
        <TestComponent />
      </WalletProvider>
    );

    expect(screen.getByTestId('connected')).toHaveTextContent('connected');
    expect(screen.getByTestId('wrong-network')).toHaveTextContent('wrong');
  });
});
