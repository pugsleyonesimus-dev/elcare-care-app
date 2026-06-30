/**
 * Unit tests for useTxToast.ts
 *
 * Covers:
 * - Success lifecycle: pushes info toasts for each phase, then a success toast
 * - Success toast includes the stellar.expert explorer URL when a hash is available
 * - Failure lifecycle: pushes an error toast with the mapped error message
 * - User rejection: shows a cancellation message, not a generic error
 * - getTxExplorerUrl: returns correct URLs for testnet / mainnet / missing hash
 * - isUserRejectionError integration: recognised phrases suppress generic errors
 */

import React from 'react';
import { render, screen, waitFor, act } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

// ── Mocks ─────────────────────────────────────────────────────────────────────

// Stub config so we control the network value
jest.mock('@/lib/config', () => ({
  config: {
    contractId: 'CTEST',
    network: 'testnet',
    rpcUrl: 'https://soroban-testnet.stellar.org',
    networkPassphrase: 'Test SDF Network ; September 2015',
    indexerUrl: '',
  },
}));

// Stub errors module — we test the real isUserRejectionError separately below
jest.mock('@/lib/errors', () => {
  const actual = jest.requireActual('@/lib/errors');
  return {
    ...actual,
    getReadableErrorMessage: (_e: unknown, fallback: string) => fallback,
  };
});

// ── Imports ───────────────────────────────────────────────────────────────────

import { useTxToast, getTxExplorerUrl } from '@/hooks/useTxToast';
import { isUserRejectionError } from '@/lib/errors';
import { ToastProvider } from '@/components/ToastProvider';

// ── Helpers ───────────────────────────────────────────────────────────────────

/**
 * Renders a minimal component that exposes the useTxToast API via data-testid
 * attributes so we can inspect it from tests.
 */
function TxToastHarness({
  fn,
  action,
  onResult,
}: {
  fn: () => Promise<unknown>;
  action?: string;
  onResult?: (result: unknown) => void;
}) {
  const { run, isRunning, phase } = useTxToast();

  const handleClick = async () => {
    const result = await run(fn, { action });
    onResult?.(result);
  };

  return (
    <div>
      <button onClick={handleClick} data-testid="trigger">
        run
      </button>
      <span data-testid="running">{String(isRunning)}</span>
      <span data-testid="phase">{phase}</span>
    </div>
  );
}

function renderHarness(
  fn: () => Promise<unknown>,
  opts: { action?: string; onResult?: (r: unknown) => void } = {}
) {
  return render(
    <ToastProvider>
      <TxToastHarness fn={fn} action={opts.action} onResult={opts.onResult} />
    </ToastProvider>
  );
}

// ── getTxExplorerUrl ──────────────────────────────────────────────────────────

describe('getTxExplorerUrl', () => {
  it('returns a testnet URL for the given hash', () => {
    const url = getTxExplorerUrl('abc123');
    expect(url).toBe('https://stellar.expert/explorer/testnet/tx/abc123');
  });

  it('returns null when hash is null', () => {
    expect(getTxExplorerUrl(null)).toBeNull();
  });

  it('returns null when hash is undefined', () => {
    expect(getTxExplorerUrl(undefined)).toBeNull();
  });

  it('returns null when hash is an empty string', () => {
    expect(getTxExplorerUrl('')).toBeNull();
  });
});

// ── isUserRejectionError ──────────────────────────────────────────────────────

describe('isUserRejectionError (from errors.ts)', () => {
  it('detects "User rejected" (Freighter)', () => {
    expect(isUserRejectionError(new Error('User rejected the request'))).toBe(true);
  });

  it('detects "user denied" (Metamask-style)', () => {
    expect(isUserRejectionError(new Error('user denied transaction'))).toBe(true);
  });

  it('detects "user cancelled"', () => {
    expect(isUserRejectionError(new Error('user cancelled the operation'))).toBe(true);
  });

  it('detects "Transaction was rejected"', () => {
    expect(isUserRejectionError(new Error('Transaction was rejected by the user'))).toBe(true);
  });

  it('returns false for a generic error', () => {
    expect(isUserRejectionError(new Error('Insufficient balance'))).toBe(false);
  });

  it('returns false for a non-Error value', () => {
    expect(isUserRejectionError(null)).toBe(false);
    expect(isUserRejectionError(42)).toBe(false);
  });

  it('works with plain strings', () => {
    expect(isUserRejectionError('Request rejected by user')).toBe(true);
    expect(isUserRejectionError('some other failure')).toBe(false);
  });
});

// ── useTxToast — success lifecycle ───────────────────────────────────────────

describe('useTxToast — success lifecycle', () => {
  beforeEach(() => jest.clearAllMocks());

  it('shows info toasts during execution and a success toast on completion', async () => {
    const user = userEvent.setup();
    const successFn = jest.fn().mockResolvedValue(undefined);

    renderHarness(successFn, { action: 'Purchase' });

    await user.click(screen.getByTestId('trigger'));

    // Success toast should appear
    await waitFor(() => {
      expect(screen.getByRole('status', { hidden: true })).toBeInTheDocument();
    });

    // The success toast message should contain the action label
    await waitFor(() => {
      const statuses = screen.getAllByRole('status', { hidden: true });
      const messages = statuses.map((el) => el.textContent ?? '');
      expect(messages.some((m) => m.includes('Purchase') && m.includes('confirmed'))).toBe(true);
    });
  });

  it('returns the result of the wrapped function', async () => {
    const user = userEvent.setup();
    const results: unknown[] = [];
    const successFn = jest.fn().mockResolvedValue(42);

    renderHarness(successFn, { action: 'Test', onResult: (r) => results.push(r) });

    await user.click(screen.getByTestId('trigger'));
    await waitFor(() => expect(results).toHaveLength(1));
    expect(results[0]).toBe(42);
  });

  it('includes explorer URL in the success toast when the result has a hash', async () => {
    const user = userEvent.setup();
    const txHash = 'deadbeef1234567890abcdef1234567890abcdef1234567890abcdef12345678';
    const successFn = jest.fn().mockResolvedValue({ hash: txHash });

    renderHarness(successFn, { action: 'Bid' });

    await user.click(screen.getByTestId('trigger'));

    await waitFor(() => {
      const statuses = screen.getAllByRole('status', { hidden: true });
      const messages = statuses.map((el) => el.textContent ?? '');
      expect(
        messages.some((m) =>
          m.includes('stellar.expert') && m.includes(txHash)
        )
      ).toBe(true);
    });
  });

  it('shows a generic success toast when the result has no hash', async () => {
    const user = userEvent.setup();
    const successFn = jest.fn().mockResolvedValue({ someField: 'value' });

    renderHarness(successFn, { action: 'Listing' });

    await user.click(screen.getByTestId('trigger'));

    await waitFor(() => {
      const statuses = screen.getAllByRole('status', { hidden: true });
      const messages = statuses.map((el) => el.textContent ?? '');
      expect(
        messages.some((m) => m.includes('Listing') && m.includes('confirmed'))
      ).toBe(true);
    });
  });

  it('isRunning is true during execution and false after', async () => {
    const user = userEvent.setup();
    let resolvePromise!: () => void;
    const controlledFn = () =>
      new Promise<void>((resolve) => {
        resolvePromise = resolve;
      });

    renderHarness(controlledFn, { action: 'Offer' });

    // Not running yet
    expect(screen.getByTestId('running').textContent).toBe('false');

    // Start the action (don't await — let it hang)
    act(() => {
      screen.getByTestId('trigger').click();
    });

    // Should be running now
    await waitFor(() =>
      expect(screen.getByTestId('running').textContent).toBe('true')
    );

    // Resolve the promise
    await act(async () => {
      resolvePromise();
    });

    // Should be done
    await waitFor(() =>
      expect(screen.getByTestId('running').textContent).toBe('false')
    );
  });

  it('uses a custom action label in toasts', async () => {
    const user = userEvent.setup();
    const successFn = jest.fn().mockResolvedValue(undefined);

    renderHarness(successFn, { action: 'My Custom Action' });
    await user.click(screen.getByTestId('trigger'));

    await waitFor(() => {
      const statuses = screen.getAllByRole('status', { hidden: true });
      const messages = statuses.map((el) => el.textContent ?? '');
      expect(messages.some((m) => m.includes('My Custom Action'))).toBe(true);
    });
  });
});

// ── useTxToast — failure lifecycle ───────────────────────────────────────────

describe('useTxToast — failure lifecycle', () => {
  beforeEach(() => jest.clearAllMocks());

  it('shows an error toast and returns null on failure', async () => {
    const user = userEvent.setup();
    const results: unknown[] = [];
    const failFn = jest.fn().mockRejectedValue(new Error('Transaction failed'));

    renderHarness(failFn, { action: 'Purchase', onResult: (r) => results.push(r) });

    await user.click(screen.getByTestId('trigger'));

    // Error toast should appear
    await waitFor(() => {
      const alerts = screen.getAllByRole('alert', { hidden: true });
      expect(alerts.length).toBeGreaterThan(0);
    });

    // run() should have returned null
    await waitFor(() => expect(results).toHaveLength(1));
    expect(results[0]).toBeNull();
  });

  it('isRunning goes back to false after failure', async () => {
    const user = userEvent.setup();
    const failFn = jest.fn().mockRejectedValue(new Error('oops'));

    renderHarness(failFn, { action: 'Bid' });

    await user.click(screen.getByTestId('trigger'));

    await waitFor(() =>
      expect(screen.getByTestId('running').textContent).toBe('false')
    );
  });

  it('phase returns to error after a failure', async () => {
    const user = userEvent.setup();
    const failFn = jest.fn().mockRejectedValue(new Error('oops'));

    renderHarness(failFn, { action: 'Bid' });

    await user.click(screen.getByTestId('trigger'));

    await waitFor(() =>
      expect(screen.getByTestId('phase').textContent).toBe('error')
    );
  });

  it('shows a user-rejection message when the wallet rejects', async () => {
    const user = userEvent.setup();
    const rejectedFn = jest
      .fn()
      .mockRejectedValue(new Error('User rejected the request'));

    renderHarness(rejectedFn, { action: 'Listing' });

    await user.click(screen.getByTestId('trigger'));

    await waitFor(() => {
      const alerts = screen.getAllByRole('alert', { hidden: true });
      const messages = alerts.map((el) => el.textContent ?? '');
      expect(
        messages.some((m) => m.includes('cancelled') || m.includes('rejected'))
      ).toBe(true);
    });
  });

  it('does NOT include explorer URL in the error toast', async () => {
    const user = userEvent.setup();
    const failFn = jest.fn().mockRejectedValue(new Error('bad tx'));

    renderHarness(failFn, { action: 'Offer' });

    await user.click(screen.getByTestId('trigger'));

    await waitFor(() => {
      const alerts = screen.getAllByRole('alert', { hidden: true });
      const messages = alerts.map((el) => el.textContent ?? '');
      expect(messages.some((m) => m.includes('stellar.expert'))).toBe(false);
    });
  });
});
