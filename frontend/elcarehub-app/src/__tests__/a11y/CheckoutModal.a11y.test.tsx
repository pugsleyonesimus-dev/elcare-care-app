import React from 'react';
import { render } from '@testing-library/react';
import { axe } from 'jest-axe';
import { CheckoutModal } from '@/components/CheckoutModal';

jest.mock('posthog-js', () => ({ capture: jest.fn() }));

jest.mock('lucide-react', () =>
  Object.fromEntries(
    ['X', 'CreditCard', 'Wallet', 'CheckCircle2', 'Loader2'].map((name) => [
      name,
      () => <span />,
    ])
  )
);

jest.mock('@/hooks/useSupportedTokens', () => ({
  useSupportedTokens: () => ({
    tokens: [
      { symbol: 'XLM', name: 'Stellar Lumens', address: 'test-xlm', decimals: 7 },
    ],
    isLoading: false,
    error: null,
    refresh: jest.fn(),
  }),
}));

jest.mock('@/lib/contract', () => ({
  ...jest.requireActual('@/lib/contract'),
  getProtocolFee: jest.fn().mockResolvedValue(250),
  stroopsToXlm: (n: bigint) => String(Number(n) / 10_000_000),
}));

const sampleListing = {
  listing_id: 1,
  price: 10_000_000n,
  metadata_cid: 'QmTest',
  status: 'Active',
  artist: 'GARTIST',
  token: 'test-xlm',
  collection: 'CCOL',
  token_id: 1,
  currency: 'XLM',
  recipients: [],
  owner: null,
  created_at: 0,
} as const;

describe('CheckoutModal accessibility', () => {
  it('has no axe violations when open', async () => {
    const { container } = render(
      <CheckoutModal
        isOpen
        onClose={() => {}}
        listing={sampleListing as never}
        onCryptoPurchase={async () => true}
        isBuyingCrypto={false}
      />
    );

    const results = await axe(container);
    expect(results).toHaveNoViolations();
  });
});
