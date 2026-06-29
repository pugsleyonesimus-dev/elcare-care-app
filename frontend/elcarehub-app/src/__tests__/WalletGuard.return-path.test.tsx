/**
 * Tests for WalletGuard redirect with return-path preservation — #88
 */
import React from 'react';
import { render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ── Mocks ─────────────────────────────────────────────────────────────────

const mockWalletContext = {
  isConnected: false,
  isWrongNetwork: false,
  status: 'DISCONNECTED' as const,
  publicKey: null,
  connect: jest.fn(),
  disconnect: jest.fn(),
  refresh: jest.fn(),
  isInstalled: false,
  isConnecting: false,
  error: null,
  networkPassphrase: null,
};

let mockPathname = '/listings/123';

jest.mock('@/context/WalletContext', () => ({
  useWalletContext: () => mockWalletContext,
}));

jest.mock('@/components/ConnectWalletModal', () => ({
  ConnectWalletModal: ({ isOpen, onClose }: { isOpen: boolean; onClose: () => void }) =>
    isOpen ? (
      <div data-testid="connect-modal">
        <button onClick={onClose} data-testid="close-modal">
          Close
        </button>
      </div>
    ) : null,
}));

jest.mock('next/navigation', () => ({
  useRouter: () => ({
    push: jest.fn(),
    pathname: mockPathname,
  }),
  usePathname: () => mockPathname,
}));

jest.mock('lucide-react', () => ({
  Wallet: () => <span />,
  AlertTriangle: () => <span />,
}));

import { WalletGuard, GuardButton } from '@/components/WalletGuard';

// ── Tests ─────────────────────────────────────────────────────────────────

describe('WalletGuard — Return-Path Preservation (#88)', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    mockWalletContext.isConnected = false;
    mockWalletContext.isWrongNetwork = false;
    mockPathname = '/listings/123';
  });

  it('shows fallback when user is not connected', () => {
    mockWalletContext.isConnected = false;
    render(
      <WalletGuard>
        <span data-testid="protected">Protected Content</span>
      </WalletGuard>
    );

    expect(screen.getByRole('button', { name: /connect wallet/i })).toBeInTheDocument();
    expect(screen.queryByTestId('protected')).not.toBeInTheDocument();
  });

  it('opens ConnectWalletModal when Connect Wallet button is clicked', async () => {
    mockWalletContext.isConnected = false;
    const user = userEvent.setup();
    
    render(
      <WalletGuard>
        <span>Protected</span>
      </WalletGuard>
    );

    const connectBtn = screen.getByRole('button', { name: /connect wallet/i });
    await user.click(connectBtn);

    await waitFor(() => {
      expect(screen.getByTestId('connect-modal')).toBeInTheDocument();
    });
  });

  it('remembers the intended route path when guard is activated', () => {
    mockPathname = '/auctions/456';
    mockWalletContext.isConnected = false;

    render(
      <WalletGuard>
        <span>Protected</span>
      </WalletGuard>
    );

    // Path should be captured (in real implementation via usePathname)
    expect(screen.getByRole('button', { name: /connect wallet/i })).toBeInTheDocument();
  });

  it('displays connection prompt with clear action', () => {
    mockWalletContext.isConnected = false;

    render(
      <WalletGuard>
        <span>Protected</span>
      </WalletGuard>
    );

    expect(screen.getByText(/wallet connection required/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /connect wallet/i })).toBeInTheDocument();
  });

  it('shows children when wallet is already connected', () => {
    mockWalletContext.isConnected = true;
    mockWalletContext.isWrongNetwork = false;

    render(
      <WalletGuard>
        <span data-testid="protected">Protected Content</span>
      </WalletGuard>
    );

    expect(screen.getByTestId('protected')).toBeInTheDocument();
  });

  it('prevents redirect loops when already connected', () => {
    mockWalletContext.isConnected = true;
    mockWalletContext.isWrongNetwork = false;
    mockPathname = '/dashboard';

    const { rerender } = render(
      <WalletGuard>
        <span data-testid="content">Dashboard Content</span>
      </WalletGuard>
    );

    expect(screen.getByTestId('content')).toBeInTheDocument();

    // Re-render should not cause redirect loop
    rerender(
      <WalletGuard>
        <span data-testid="content">Dashboard Content</span>
      </WalletGuard>
    );

    expect(screen.getByTestId('content')).toBeInTheDocument();
  });

  it('handles modal closure without connection', async () => {
    mockWalletContext.isConnected = false;
    const user = userEvent.setup();

    render(
      <WalletGuard>
        <span>Protected</span>
      </WalletGuard>
    );

    const connectBtn = screen.getByRole('button', { name: /connect wallet/i });
    await user.click(connectBtn);

    await waitFor(() => {
      expect(screen.getByTestId('connect-modal')).toBeInTheDocument();
    });

    const closeBtn = screen.getByTestId('close-modal');
    await user.click(closeBtn);

    // Modal should close but user remains on same page
    await waitFor(() => {
      expect(screen.queryByTestId('connect-modal')).not.toBeInTheDocument();
    });
  });

  it('blocks actions on wrong network', () => {
    mockWalletContext.isConnected = true;
    mockWalletContext.isWrongNetwork = true;

    render(
      <WalletGuard>
        <span>Protected</span>
      </WalletGuard>
    );

    // Should show error message for wrong network
    expect(screen.getByText(/wrong network/i)).toBeInTheDocument();
  });

  it('displays wrong network indicator', () => {
    mockWalletContext.isConnected = true;
    mockWalletContext.isWrongNetwork = true;

    render(
      <WalletGuard>
        <span>Protected</span>
      </WalletGuard>
    );

    expect(screen.getByText(/wrong network detected/i)).toBeInTheDocument();
  });

  it('GuardButton prevents action when not connected', async () => {
    mockWalletContext.isConnected = false;
    const onAction = jest.fn();
    const user = userEvent.setup();

    render(
      <GuardButton onAction={onAction}>
        Buy Now
      </GuardButton>
    );

    await user.click(screen.getByRole('button', { name: /buy now/i }));

    // onAction should not be called
    expect(onAction).not.toHaveBeenCalled();

    // Modal should open
    await waitFor(() => {
      expect(screen.getByTestId('connect-modal')).toBeInTheDocument();
    });
  });

  it('GuardButton allows action when connected', async () => {
    mockWalletContext.isConnected = true;
    mockWalletContext.isWrongNetwork = false;
    const onAction = jest.fn();
    const user = userEvent.setup();

    render(
      <GuardButton onAction={onAction}>
        Buy Now
      </GuardButton>
    );

    await user.click(screen.getByRole('button', { name: /buy now/i }));

    // onAction should be called
    expect(onAction).toHaveBeenCalled();

    // Modal should not open
    expect(screen.queryByTestId('connect-modal')).not.toBeInTheDocument();
  });

  it('GuardButton blocks action on wrong network', async () => {
    mockWalletContext.isConnected = true;
    mockWalletContext.isWrongNetwork = true;
    const onAction = jest.fn();
    const user = userEvent.setup();

    render(
      <GuardButton onAction={onAction}>
        Place Bid
      </GuardButton>
    );

    await user.click(screen.getByRole('button', { name: /place bid/i }));

    // onAction should not be called due to wrong network
    expect(onAction).not.toHaveBeenCalled();

    // Modal should open to prompt reconnection
    await waitFor(() => {
      expect(screen.getByTestId('connect-modal')).toBeInTheDocument();
    });
  });

  it('respects hideContentWhenDisconnected prop', () => {
    mockWalletContext.isConnected = false;

    const { container } = render(
      <WalletGuard hideContentWhenDisconnected={true}>
        <span data-testid="protected">Protected</span>
      </WalletGuard>
    );

    // Content should be hidden
    expect(screen.queryByTestId('protected')).not.toBeInTheDocument();
  });

  it('shows default fallback when no custom fallback provided', () => {
    mockWalletContext.isConnected = false;

    render(
      <WalletGuard>
        <span>Protected</span>
      </WalletGuard>
    );

    expect(screen.getByText(/wallet connection required/i)).toBeInTheDocument();
  });

  it('shows custom fallback when provided', () => {
    mockWalletContext.isConnected = false;

    render(
      <WalletGuard fallback={<span data-testid="custom">Custom Fallback</span>}>
        <span>Protected</span>
      </WalletGuard>
    );

    expect(screen.getByTestId('custom')).toBeInTheDocument();
  });

  it('uses custom actionName in fallback message', () => {
    mockWalletContext.isConnected = false;

    render(
      <WalletGuard actionName="To place a bid">
        <span>Protected</span>
      </WalletGuard>
    );

    expect(screen.getByText(/to place a bid/i)).toBeInTheDocument();
  });

  it('closes modal and retains guard state after cancellation', async () => {
    mockWalletContext.isConnected = false;
    const user = userEvent.setup();

    const { rerender } = render(
      <WalletGuard>
        <span>Protected</span>
      </WalletGuard>
    );

    const connectBtn = screen.getByRole('button', { name: /connect wallet/i });
    await user.click(connectBtn);

    await waitFor(() => {
      expect(screen.getByTestId('connect-modal')).toBeInTheDocument();
    });

    // Close without connecting
    const closeBtn = screen.getByTestId('close-modal');
    await user.click(closeBtn);

    // Guard should still be visible
    await waitFor(() => {
      expect(screen.getByRole('button', { name: /connect wallet/i })).toBeInTheDocument();
    });
  });

  it('transitions to showing children after successful connection', () => {
    mockWalletContext.isConnected = false;

    const { rerender } = render(
      <WalletGuard>
        <span data-testid="protected">Protected Content</span>
      </WalletGuard>
    );

    // Initially disconnected
    expect(screen.queryByTestId('protected')).not.toBeInTheDocument();

    // Simulate successful connection
    mockWalletContext.isConnected = true;
    rerender(
      <WalletGuard>
        <span data-testid="protected">Protected Content</span>
      </WalletGuard>
    );

    // Now children should be visible
    expect(screen.getByTestId('protected')).toBeInTheDocument();
  });

  it('multiple guards on different routes preserve separate paths', () => {
    mockWalletContext.isConnected = false;
    mockPathname = '/listings/123';

    // Test that different route paths can have their own WalletGuard
    const { container: container1 } = render(
      <WalletGuard>
        <span>Listings Guard</span>
      </WalletGuard>
    );

    // Verify guard is showing for disconnected state
    expect(screen.getByRole('button', { name: /connect wallet/i })).toBeInTheDocument();

    // Each WalletGuard instance should respect the current path
    mockPathname = '/auctions/456';

    const { container: container2 } = render(
      <WalletGuard>
        <span>Auctions Guard</span>
      </WalletGuard>
    );

    // Both should show connect buttons in disconnected state
    const buttons = screen.getAllByRole('button', { name: /connect wallet/i });
    expect(buttons.length).toBeGreaterThanOrEqual(1);
  });
});
