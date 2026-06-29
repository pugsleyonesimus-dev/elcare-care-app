/**
 * Page-level tests for app/offers/page.tsx — Outgoing Offers Dashboard.
 */
import React from 'react';
import { render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ── Mocks ─────────────────────────────────────────────────────────────────────

const mockWithdraw = jest.fn();
const mockRefresh = jest.fn();

jest.mock('@/context/WalletContext', () => ({
  useWalletContext: () => ({
    publicKey: 'GPUBKEY',
    isConnected: true,
    isWrongNetwork: false,
    status: 'connected',
    connect: jest.fn(),
    disconnect: jest.fn(),
    refresh: jest.fn(),
    isInstalled: true,
    isConnecting: false,
    error: null,
    networkPassphrase: null,
  }),
}));

jest.mock('@/components/WalletGuard', () => ({
  WalletGuard: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

jest.mock('@/components/ConnectWalletModal', () => ({
  ConnectWalletModal: () => null,
}));

// Default: return offers
const mockUseOffererOffers = jest.fn();
const mockUseWithdrawOffer = jest.fn();

jest.mock('@/hooks/useOffers', () => ({
  useOffererOffers: (...args: unknown[]) => mockUseOffererOffers(...args),
  useWithdrawOffer: (...args: unknown[]) => mockUseWithdrawOffer(...args),
}));

jest.mock('@/lib/contract', () => ({
  stroopsToXlm: (s: bigint) => String(Number(s) / 10_000_000),
}));

jest.mock('@/config/tokens', () => ({
  SUPPORTED_TOKENS: [
    { symbol: 'XLM', name: 'Stellar Lumens', address: 'CTOKEN_XLM', decimals: 7 },
  ],
}));

jest.mock('clsx', () => ({
  clsx: (...args: unknown[]) => args.filter(Boolean).join(' '),
}));

// Stub lucide-react icons
jest.mock('lucide-react', () => {
  const icon = (name: string) =>
    function MockIcon(props: Record<string, unknown>) {
      return <span data-testid={`icon-${name}`} />;
    };
  return {
    ShoppingBag: icon('ShoppingBag'),
    Clock: icon('Clock'),
    CheckCircle: icon('CheckCircle'),
    XCircle: icon('XCircle'),
    ArrowUpRight: icon('ArrowUpRight'),
    History: icon('History'),
    Activity: icon('Activity'),
    TrendingUp: icon('TrendingUp'),
    Loader2: icon('Loader2'),
    Inbox: icon('Inbox'),
    CalendarClock: icon('CalendarClock'),
  };
});

import OffersPage from '@/app/offers/page';

// ── Helpers ───────────────────────────────────────────────────────────────────

function makeOffer(id: number, overrides: Record<string, unknown> = {}) {
  return {
    offer_id: id,
    listing_id: 1,
    offerer: 'GOFFERER',
    amount: 5_000_000n,
    token: 'CTOKEN_XLM',
    status: 'Pending',
    created_at: 1700000000,
    listing: {
      listing_id: 1,
      artist: 'GARTIST',
      status: 'Active',
      price: 10_000_000n,
      metadata_cid: 'Qm',
      expires_at: undefined,
    },
    ...overrides,
  };
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('OffersPage', () => {
  beforeEach(() => {
    jest.clearAllMocks();
    mockUseWithdrawOffer.mockReturnValue({
      withdraw: mockWithdraw,
      isWithdrawing: false,
      error: null,
    });
  });

  it('renders loading skeletons when isLoading is true', () => {
    mockUseOffererOffers.mockReturnValue({
      offers: [],
      isLoading: true,
      error: null,
      refresh: mockRefresh,
    });

    render(<OffersPage />);
    // Skeleton placeholders are rendered as pulse divs
    const skeletons = document.querySelectorAll('.animate-pulse');
    expect(skeletons.length).toBeGreaterThanOrEqual(1);
  });

  it('renders empty state when no offers', () => {
    mockUseOffererOffers.mockReturnValue({
      offers: [],
      isLoading: false,
      error: null,
      refresh: mockRefresh,
    });

    render(<OffersPage />);
    expect(screen.getByText('No offers yet.')).toBeInTheDocument();
    expect(screen.getByText('Browse listings')).toBeInTheDocument();
  });

  it('renders offer cards with status, amount, token, and listing link', () => {
    const offers = [
      makeOffer(10, { status: 'Pending' }),
      makeOffer(11, { status: 'Accepted' }),
    ];

    mockUseOffererOffers.mockReturnValue({
      offers,
      isLoading: false,
      error: null,
      refresh: mockRefresh,
    });

    render(<OffersPage />);

    // Both cards rendered
    expect(screen.getByTestId('offer-card-10')).toBeInTheDocument();
    expect(screen.getByTestId('offer-card-11')).toBeInTheDocument();

    // Status badges
    expect(screen.getByText('Pending')).toBeInTheDocument();
    expect(screen.getByText('Accepted')).toBeInTheDocument();

    // Amount displayed (stroopsToXlm mock returns "0.5")
    const amounts = screen.getAllByText('0.5');
    expect(amounts.length).toBe(2);

    // Token symbol
    const tokenLabels = screen.getAllByText('XLM');
    expect(tokenLabels.length).toBe(2);

    // Listing links
    const listingLinks = screen.getAllByText('#1');
    expect(listingLinks.length).toBeGreaterThanOrEqual(2);
  });

  it('displays listing expiry when present', () => {
    const futureTs = Math.floor(Date.now() / 1000) + 86400;
    const offers = [
      makeOffer(20, {
        listing: {
          listing_id: 1,
          artist: 'GARTIST',
          status: 'Active',
          price: 10_000_000n,
          metadata_cid: 'Qm',
          expires_at: futureTs,
        },
      }),
    ];

    mockUseOffererOffers.mockReturnValue({
      offers,
      isLoading: false,
      error: null,
      refresh: mockRefresh,
    });

    render(<OffersPage />);
    const expiryEl = screen.getByTestId('offer-expiry-20');
    expect(expiryEl).toBeInTheDocument();
    // Should NOT say "No expiry" since we have an expires_at value
    expect(expiryEl.textContent).not.toContain('No expiry');
  });

  it('displays "No expiry" when listing has no expires_at', () => {
    const offers = [makeOffer(21)];

    mockUseOffererOffers.mockReturnValue({
      offers,
      isLoading: false,
      error: null,
      refresh: mockRefresh,
    });

    render(<OffersPage />);
    const expiryEl = screen.getByTestId('offer-expiry-21');
    expect(expiryEl.textContent).toContain('No expiry');
  });

  it('withdraw button calls withdraw and triggers refresh', async () => {
    mockWithdraw.mockResolvedValueOnce(true);
    const offers = [makeOffer(30, { status: 'Pending' })];

    mockUseOffererOffers.mockReturnValue({
      offers,
      isLoading: false,
      error: null,
      refresh: mockRefresh,
    });

    const user = userEvent.setup();
    render(<OffersPage />);

    const btn = screen.getByTestId('withdraw-btn-30');
    await user.click(btn);

    await waitFor(() => expect(mockWithdraw).toHaveBeenCalledWith(30));
    await waitFor(() => expect(mockRefresh).toHaveBeenCalled());
  });

  it('does not show withdraw button for non-Pending offers', () => {
    const offers = [makeOffer(31, { status: 'Accepted' })];

    mockUseOffererOffers.mockReturnValue({
      offers,
      isLoading: false,
      error: null,
      refresh: mockRefresh,
    });

    render(<OffersPage />);
    expect(screen.queryByTestId('withdraw-btn-31')).not.toBeInTheDocument();
  });

  it('tab filtering shows only matching status', async () => {
    const offers = [
      makeOffer(40, { status: 'Pending' }),
      makeOffer(41, { status: 'Accepted' }),
    ];

    mockUseOffererOffers.mockReturnValue({
      offers,
      isLoading: false,
      error: null,
      refresh: mockRefresh,
    });

    const user = userEvent.setup();
    render(<OffersPage />);

    // All tab — both visible
    expect(screen.getByTestId('offer-card-40')).toBeInTheDocument();
    expect(screen.getByTestId('offer-card-41')).toBeInTheDocument();

    // Click "Accepted" tab
    await user.click(screen.getByText('Accepted'));

    // Only accepted card visible
    expect(screen.queryByTestId('offer-card-40')).not.toBeInTheDocument();
    expect(screen.getByTestId('offer-card-41')).toBeInTheDocument();
  });

  it('renders cross-navigation link to offer inbox', () => {
    mockUseOffererOffers.mockReturnValue({
      offers: [],
      isLoading: false,
      error: null,
      refresh: mockRefresh,
    });

    render(<OffersPage />);
    const navLink = screen.getByTestId('nav-incoming');
    expect(navLink).toBeInTheDocument();
    expect(navLink).toHaveAttribute('href', '/offers/incoming');
  });

  it('displays correct stats counters', () => {
    const offers = [
      makeOffer(50, { status: 'Pending' }),
      makeOffer(51, { status: 'Pending' }),
      makeOffer(52, { status: 'Accepted' }),
    ];

    mockUseOffererOffers.mockReturnValue({
      offers,
      isLoading: false,
      error: null,
      refresh: mockRefresh,
    });

    render(<OffersPage />);
    const stats = screen.getByTestId('stats-grid');
    // Total Placed: 3, Pending Response: 2, Successfully Accepted: 1
    expect(within(stats).getByText('3')).toBeInTheDocument();
    expect(within(stats).getByText('2')).toBeInTheDocument();
    expect(within(stats).getByText('1')).toBeInTheDocument();
  });

  it('renders error banner when there is an error', () => {
    mockUseOffererOffers.mockReturnValue({
      offers: [],
      isLoading: false,
      error: 'Something went wrong',
      refresh: mockRefresh,
    });

    render(<OffersPage />);
    expect(screen.getByText('Something went wrong')).toBeInTheDocument();
  });
});
