/**
 * Component tests for ConnectWalletModal.
 */
import React from 'react';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// -- Mocks --
const mockConnect = jest.fn();
const mockRefresh = jest.fn();
let mockStatus = 'DISCONNECTED';
let mockIsConnecting = false;
let mockError: string | null = null;
let mockPublicKey: string | null = null;

jest.mock('@/context/WalletContext', () => ({
  useWalletContext: () => ({
    status: mockStatus,
    connect: mockConnect,
    isConnecting: mockIsConnecting,
    isConnected: mockStatus === 'CONNECTED',
    isWrongNetwork: false,
    error: mockError,
    publicKey: mockPublicKey,
    refresh: mockRefresh,
    walletType: null,
    networkPassphrase: null,
    isInstalled: false,
    disconnect: jest.fn(),
    connectFreighter: mockConnect,
    connectLobstr: jest.fn(),
    freighter: { isInstalled: false, isConnecting: false, isConnected: false, isWrongNetwork: false, error: null },
    lobstr: { isInstalled: false, isConnecting: false, isConnected: false, isWrongNetwork: false, error: null },
    magic: { isConnecting: false, isConnected: false, error: null, publicAddress: null, logout: jest.fn() },
  }),
}));

jest.mock('@/components/MagicWalletModal', () => ({
  MagicWalletModal: ({ isOpen }: { isOpen: boolean }) =>
    isOpen ? <div data-testid="magic-modal" /> : null,
}));

jest.mock('@/lib/config', () => ({
  config: { network: 'testnet', contractId: 'CTEST123' },
}));

jest.mock('posthog-js', () => ({
  capture: jest.fn(),
}));

jest.mock('lucide-react', () =>
  Object.fromEntries(
    ['X', 'Wallet', 'ExternalLink', 'ShieldCheck', 'AlertTriangle',
      'ArrowRight', 'Loader2', 'CheckCircle2', 'Mail']
      .map((name) => [name, () => <span data-testid={name} />])
  )
);

import { ConnectWalletModal } from '@/components/ConnectWalletModal';

// -- Tests --
describe('ConnectWalletModal', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    mockStatus = 'DISCONNECTED';
    mockIsConnecting = false;
    mockError = null;
    mockPublicKey = null;
  });

  it('renders nothing when isOpen is false', () => {
    const { container } = render(
      <ConnectWalletModal isOpen={false} onClose={jest.fn()} />
    );
    expect(container.firstChild).toBeNull();
  });

  it('renders the modal with Freighter option', () => {
    render(<ConnectWalletModal isOpen={true} onClose={jest.fn()} />);
    expect(screen.getByText(/freighter/i)).toBeInTheDocument();
  });

  it('calls onClose when the backdrop is clicked', async () => {
    const onClose = jest.fn();
    const user = userEvent.setup();
    const { container } = render(
      <ConnectWalletModal isOpen={true} onClose={onClose} />
    );
    const backdrop = container.querySelector('.absolute.inset-0');
    if (backdrop) {
      await user.click(backdrop);
      expect(onClose).toHaveBeenCalled();
    }
  });

  it('calls connect when the Freighter button is clicked', async () => {
    const user = userEvent.setup();
    render(<ConnectWalletModal isOpen={true} onClose={jest.fn()} />);
    const connectBtn = screen.getByRole('button', { name: /freighter/i });
    await user.click(connectBtn);
    expect(mockConnect).toHaveBeenCalled();
  });

  it('shows error message when error is set', () => {
    mockError = 'Connection rejected';
    render(<ConnectWalletModal isOpen={true} onClose={jest.fn()} />);
    expect(screen.getByText(/connection rejected/i)).toBeInTheDocument();
  });

  it('shows Magic Wallet option', () => {
    render(<ConnectWalletModal isOpen={true} onClose={jest.fn()} />);
    expect(screen.getByText(/magic wallet/i)).toBeInTheDocument();
  });
});