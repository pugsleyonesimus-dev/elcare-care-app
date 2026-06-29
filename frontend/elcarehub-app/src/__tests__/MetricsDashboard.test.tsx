/**
 * Tests for the MetricsDashboard tab inside DashboardPage.
 * Verifies KPI cards render with seeded aggregates and graceful empty state.
 */
import React from 'react';
import { render, screen, waitFor, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ── Mocks ─────────────────────────────────────────────────────────────────────

const mockFetchArtistMetrics = jest.fn();

jest.mock('@/lib/indexer', () => ({
  fetchArtistMetrics: (...args: unknown[]) => mockFetchArtistMetrics(...args),
  fetchArtistListings: jest.fn().mockResolvedValue([]),
}));

jest.mock('@/lib/contract', () => ({
  getArtistListings: jest.fn().mockResolvedValue([]),
  cancelListing: jest.fn(),
  stroopsToXlm: () => '10',
}));

jest.mock('@/context/WalletContext', () => ({
  useWalletContext: () => ({ publicKey: 'GARTIST123' }),
}));

jest.mock('@/hooks/useMarketplace', () => ({
  useArtistListings: () => ({ listings: [], isLoading: false, refresh: jest.fn() }),
  useCancelListing: () => ({ cancel: jest.fn(), isCancelling: false }),
}));

jest.mock('@/components/WalletGuard', () => ({
  WalletGuard: ({ children }: { children: React.ReactNode }) => <>{children}</>,
}));

jest.mock('@/components/ListingForm', () => ({
  ListingForm: () => <div>ListingForm</div>,
}));

jest.mock('@/components/AuctionForm', () => ({
  AuctionForm: () => <div>AuctionForm</div>,
}));

jest.mock('@/config/tokens', () => ({
  SUPPORTED_TOKENS: [],
}));

import DashboardPage from '@/app/dashboard/page';

// ── Helpers ───────────────────────────────────────────────────────────────────

function seedMetrics(overrides = {}) {
  return {
    address: 'GARTIST123',
    range: 'week',
    totalListings: 5,
    totalSales: 3,
    totalVolume: '300000000',
    uniqueBuyers: 2,
    conversionRate: 0.6,
    salesTimeline: [
      { date: '2026-06-22', count: 1 },
      { date: '2026-06-23', count: 2 },
    ],
    ...overrides,
  };
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe('MetricsDashboard tab', () => {
  beforeEach(() => jest.clearAllMocks());

  it('renders KPI cards with seeded aggregate data', async () => {
    mockFetchArtistMetrics.mockResolvedValue(seedMetrics());

    render(<DashboardPage />);

    await act(async () => {
      await userEvent.click(screen.getByRole('button', { name: /metrics/i }));
    });

    await waitFor(() => {
      expect(screen.getByText('5')).toBeInTheDocument(); // totalListings
      expect(screen.getByText('3')).toBeInTheDocument(); // totalSales
      expect(screen.getByText('2')).toBeInTheDocument(); // uniqueBuyers
      expect(screen.getByText('60.0%')).toBeInTheDocument(); // conversionRate
    });
  });

  it('shows empty chart message when salesTimeline is empty', async () => {
    mockFetchArtistMetrics.mockResolvedValue(seedMetrics({ salesTimeline: [] }));

    render(<DashboardPage />);

    await act(async () => {
      await userEvent.click(screen.getByRole('button', { name: /metrics/i }));
    });

    await waitFor(() => {
      expect(screen.getByText(/no.*sales over time.*data yet/i)).toBeInTheDocument();
    });
  });

  it('shows loading skeletons while fetching', async () => {
    let resolve!: (v: ReturnType<typeof seedMetrics>) => void;
    mockFetchArtistMetrics.mockReturnValue(new Promise((r) => { resolve = r; }));

    render(<DashboardPage />);

    await act(async () => {
      await userEvent.click(screen.getByRole('button', { name: /metrics/i }));
    });

    // Skeletons present before resolve
    expect(document.querySelectorAll('.animate-pulse').length).toBeGreaterThan(0);

    await act(async () => { resolve(seedMetrics()); });
  });

  it('calls fetchArtistMetrics with new range when range button clicked', async () => {
    mockFetchArtistMetrics.mockResolvedValue(seedMetrics());

    render(<DashboardPage />);

    await act(async () => {
      await userEvent.click(screen.getByRole('button', { name: /metrics/i }));
    });

    await waitFor(() => expect(mockFetchArtistMetrics).toHaveBeenCalledWith('GARTIST123', 'week'));

    await act(async () => {
      await userEvent.click(screen.getByRole('button', { name: /^month$/i }));
    });

    await waitFor(() => expect(mockFetchArtistMetrics).toHaveBeenCalledWith('GARTIST123', 'month'));
  });
});
