/**
 * Tests for RootErrorBoundary component.
 */
import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";

// ── Mocks ─────────────────────────────────────────────────────────────────────

jest.mock("@sentry/nextjs", () => ({
  withScope: (fn: (scope: unknown) => void) =>
    fn({ setContext: jest.fn() }),
  captureException: jest.fn().mockReturnValue("mock-sentry-event-id"),
  showReportDialog: jest.fn(),
}));

jest.mock("lucide-react", () =>
  Object.fromEntries(
    ["AlertTriangle", "RefreshCw"].map((name) => [name, () => <span />])
  )
);

import { RootErrorBoundary } from "@/components/RootErrorBoundary";
import * as Sentry from "@sentry/nextjs";

function Bomb({ shouldThrow }: { shouldThrow: boolean }) {
  if (shouldThrow) throw new Error("Test explosion");
  return <div>Safe content</div>;
}

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("RootErrorBoundary", () => {
  beforeEach(() => {
    jest.clearAllMocks();
    jest.spyOn(console, "error").mockImplementation(() => {});
  });

  afterEach(() => {
    (console.error as jest.Mock).mockRestore();
  });

  it("renders children when no error is thrown", () => {
    render(
      <RootErrorBoundary>
        <Bomb shouldThrow={false} />
      </RootErrorBoundary>
    );
    expect(screen.getByText("Safe content")).toBeInTheDocument();
  });

  it("renders error heading when child throws", () => {
    render(
      <RootErrorBoundary>
        <Bomb shouldThrow />
      </RootErrorBoundary>
    );
    expect(screen.getByText(/something went wrong/i)).toBeInTheDocument();
  });

  it("displays the thrown error message", () => {
    render(
      <RootErrorBoundary>
        <Bomb shouldThrow />
      </RootErrorBoundary>
    );
    expect(screen.getByText(/Test explosion/)).toBeInTheDocument();
  });

  it("renders a Reload app button", () => {
    render(
      <RootErrorBoundary>
        <Bomb shouldThrow />
      </RootErrorBoundary>
    );
    expect(screen.getByRole("button", { name: /reload app/i })).toBeInTheDocument();
  });

  it("renders a Report issue button after Sentry captures the event", () => {
    render(
      <RootErrorBoundary>
        <Bomb shouldThrow />
      </RootErrorBoundary>
    );
    expect(screen.getByRole("button", { name: /report issue/i })).toBeInTheDocument();
  });

  it("calls Sentry.captureException when an error is caught", () => {
    render(
      <RootErrorBoundary>
        <Bomb shouldThrow />
      </RootErrorBoundary>
    );
    expect(Sentry.captureException).toHaveBeenCalledTimes(1);
  });

  it("clicking Reload app does not throw", () => {
    // JSDOM's window.location.reload is non-configurable so we verify the
    // button click does not throw rather than asserting reload was called.
    render(
      <RootErrorBoundary>
        <Bomb shouldThrow />
      </RootErrorBoundary>
    );
    expect(() =>
      fireEvent.click(screen.getByRole("button", { name: /reload app/i }))
    ).not.toThrow();
  });

  it("calls Sentry.showReportDialog when Report issue is clicked", () => {
    render(
      <RootErrorBoundary>
        <Bomb shouldThrow />
      </RootErrorBoundary>
    );

    fireEvent.click(screen.getByRole("button", { name: /report issue/i }));
    expect(Sentry.showReportDialog).toHaveBeenCalledWith({
      eventId: "mock-sentry-event-id",
    });
  });
});
