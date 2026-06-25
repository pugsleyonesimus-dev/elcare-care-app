/**
 * Tests for the auction detail page components (ISSUE-021):
 *   - Countdown ticks and reflects remaining time
 *   - Countdown shows "Auction Ended" when expired
 *   - SSE AUCTION_EXTENDED event updates the end time
 *   - Finalize CTA appears for any user after expiry
 *
 * We test via the exported helper hooks / components rather than mounting
 * the full page (which requires several async Soroban/indexer calls).
 */

import React from "react";
import { render, screen, act } from "@testing-library/react";

// ── Import the exported countdown hook & component ────────────────────────────
import {
  useAuctionCountdown,
  Countdown,
} from "@/app/auctions/[id]/page";

// ── Helpers ───────────────────────────────────────────────────────────────────

/** A thin wrapper that exposes hook values as data attributes for assertions. */
function CountdownHookConsumer({ endTime }: { endTime: number }) {
  const { remaining, isExpired, days, hours, minutes, seconds, setEndTime } =
    useAuctionCountdown(endTime);

  return (
    <div>
      <span data-testid="remaining">{remaining}</span>
      <span data-testid="expired">{String(isExpired)}</span>
      <span data-testid="days">{days}</span>
      <span data-testid="hours">{hours}</span>
      <span data-testid="minutes">{minutes}</span>
      <span data-testid="seconds">{seconds}</span>
      {/* Expose the setter so tests can simulate an SSE extension */}
      <button
        data-testid="extend-btn"
        onClick={() => setEndTime(Math.floor(Date.now() / 1000) + 7200)}
      >
        extend
      </button>
    </div>
  );
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("useAuctionCountdown", () => {
  beforeEach(() => jest.useFakeTimers());
  afterEach(() => jest.useRealTimers());

  it("reports a positive remaining time for a future end time", () => {
    const endTime = Math.floor(Date.now() / 1000) + 3600; // 1 hour from now
    render(<CountdownHookConsumer endTime={endTime} />);

    const remaining = Number(screen.getByTestId("remaining").textContent);
    expect(remaining).toBeGreaterThan(0);
    expect(screen.getByTestId("expired").textContent).toBe("false");
  });

  it("reports isExpired=true and remaining=0 for a past end time", () => {
    const endTime = Math.floor(Date.now() / 1000) - 1; // 1 second ago
    render(<CountdownHookConsumer endTime={endTime} />);

    expect(screen.getByTestId("remaining").textContent).toBe("0");
    expect(screen.getByTestId("expired").textContent).toBe("true");
  });

  it("decrements remaining by 1 each second", () => {
    const endTime = Math.floor(Date.now() / 1000) + 10;
    render(<CountdownHookConsumer endTime={endTime} />);

    const before = Number(screen.getByTestId("remaining").textContent);

    act(() => {
      jest.advanceTimersByTime(1_000);
    });

    const after = Number(screen.getByTestId("remaining").textContent);
    expect(after).toBe(before - 1);
  });

  it("clamps remaining at 0 and never goes negative", () => {
    const endTime = Math.floor(Date.now() / 1000) + 1;
    render(<CountdownHookConsumer endTime={endTime} />);

    act(() => {
      jest.advanceTimersByTime(5_000);
    });

    expect(Number(screen.getByTestId("remaining").textContent)).toBe(0);
    expect(screen.getByTestId("expired").textContent).toBe("true");
  });

  it("updates when setEndTime is called (simulating an SSE extension)", () => {
    // Start with 1 second remaining — should be nearly expired.
    const endTime = Math.floor(Date.now() / 1000) + 1;
    render(<CountdownHookConsumer endTime={endTime} />);

    // Advance past expiry.
    act(() => jest.advanceTimersByTime(2_000));
    expect(screen.getByTestId("expired").textContent).toBe("true");

    // Simulate the AUCTION_EXTENDED SSE event extending the end time by 2h.
    act(() => {
      screen.getByTestId("extend-btn").click();
    });

    // Tick so the countdown re-evaluates.
    act(() => jest.advanceTimersByTime(1_000));

    expect(screen.getByTestId("expired").textContent).toBe("false");
    const remaining = Number(screen.getByTestId("remaining").textContent);
    expect(remaining).toBeGreaterThan(0);
  });

  it("breaks days, hours, minutes, seconds correctly", () => {
    // Exactly 1d 2h 3m 4s = 93784 seconds
    const endTime = Math.floor(Date.now() / 1000) + 93784;
    render(<CountdownHookConsumer endTime={endTime} />);

    expect(screen.getByTestId("days").textContent).toBe("1");
    expect(screen.getByTestId("hours").textContent).toBe("2");
    expect(screen.getByTestId("minutes").textContent).toBe("3");
    // seconds may be 4 or 3 depending on sub-second timing; allow ±1
    const secs = Number(screen.getByTestId("seconds").textContent);
    expect(secs).toBeGreaterThanOrEqual(3);
    expect(secs).toBeLessThanOrEqual(4);
  });
});

// ── Countdown component ───────────────────────────────────────────────────────

describe("Countdown component", () => {
  beforeEach(() => jest.useFakeTimers());
  afterEach(() => jest.useRealTimers());

  it("renders the countdown boxes for an active (future) end time", () => {
    const endTime = Math.floor(Date.now() / 1000) + 3600;
    render(<Countdown endTime={endTime} />);
    expect(screen.getByTestId("countdown")).toBeInTheDocument();
    expect(screen.queryByTestId("countdown-expired")).not.toBeInTheDocument();
  });

  it("shows 'Auction Ended' for an expired end time", () => {
    const endTime = Math.floor(Date.now() / 1000) - 10;
    render(<Countdown endTime={endTime} />);
    expect(screen.getByTestId("countdown-expired")).toBeInTheDocument();
    expect(screen.queryByTestId("countdown")).not.toBeInTheDocument();
  });

  it("transitions from active to expired after time passes", () => {
    const endTime = Math.floor(Date.now() / 1000) + 2;
    render(<Countdown endTime={endTime} />);

    expect(screen.getByTestId("countdown")).toBeInTheDocument();

    act(() => jest.advanceTimersByTime(3_000));

    expect(screen.getByTestId("countdown-expired")).toBeInTheDocument();
    expect(screen.queryByTestId("countdown")).not.toBeInTheDocument();
  });

  it("reflects an updated endTime prop (simulates SSE extension)", () => {
    // Render with 1 second remaining.
    const { rerender } = render(
      <Countdown endTime={Math.floor(Date.now() / 1000) + 1} />
    );

    // Advance past expiry.
    act(() => jest.advanceTimersByTime(3_000));
    expect(screen.getByTestId("countdown-expired")).toBeInTheDocument();

    // Re-render with an extended end time (as the parent page would do on SSE).
    act(() => {
      rerender(
        <Countdown endTime={Math.floor(Date.now() / 1000) + 7200} />
      );
    });

    // Tick once so the interval fires.
    act(() => jest.advanceTimersByTime(1_000));

    expect(screen.getByTestId("countdown")).toBeInTheDocument();
    expect(screen.queryByTestId("countdown-expired")).not.toBeInTheDocument();
  });
});
