import React from 'react';
import { render } from '@testing-library/react';
import { axe } from 'jest-axe';
import { ConnectWalletModal } from '@/components/ConnectWalletModal';

jest.mock('posthog-js', () => ({ capture: jest.fn() }));

jest.mock('@/context/WalletContext', () => ({
  useWalletContext: () => ({
    isConnected: false,
    publicKey: null,
    refresh: jest.fn(),
    freighter: {
      isConnecting: false,
      isInstalled: true,
      isWrongNetwork: false,
      error: null,
    },
    lobstr: {
      isConnecting: false,
      isInstalled: true,
      isWrongNetwork: false,
      error: null,
    },
    magic: { isConnecting: false, isConnected: false, error: null },
    connectFreighter: jest.fn(),
    connectLobstr: jest.fn(),
  }),
}));

jest.mock('@/components/MagicWalletModal', () => ({
  MagicWalletModal: () => null,
}));

jest.mock('lucide-react', () =>
  Object.fromEntries(
    [
      'X',
      'Wallet',
      'ExternalLink',
      'ShieldCheck',
      'AlertTriangle',
      'ArrowRight',
      'Loader2',
      'CheckCircle2',
      'Mail',
    ].map((name) => [name, () => <span />])
  )
);

describe('ConnectWalletModal accessibility', () => {
  it('has no axe violations when open', async () => {
    const { container } = render(
      <ConnectWalletModal isOpen onClose={() => {}} />
    );

    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});
