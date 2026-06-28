/**
 * Tests for ProvenanceTimeline (Issue #99).
 *
 * Covers:
 * - Loading state
 * - Error state
 * - Empty history
 * - Rendering multiple event types with correct labels
 * - Actor profile links
 * - Transaction explorer links (present for real hashes, absent for ledger_ ids)
 * - Chronological ordering
 * - Load more / pagination
 */

import React from "react";
import { render, screen, fireEvent } from "@testing-library/react";
import { ProvenanceTimeline, ProvenanceTimelineProps } from "@/components/ProvenanceTimeline";
import { ActivityEvent } from "@/lib/indexer";

// ── Mock Next.js Link ─────────────────────────────────────────────────────────
jest.mock("next/link", () => {
  const MockLink = ({ href, children, ...props }: any) => (
    <a href={href} {...props}>
      {children}
    </a>
  );
  MockLink.displayName = "MockLink";
  return MockLink;
});

// ── Mock config for explorer URL ──────────────────────────────────────────────
jest.mock("@/lib/config", () => ({
  config: { network: "testnet", contractId: "CONTRACT123" },
}));

// ── Test fixtures ─────────────────────────────────────────────────────────────

const ADDR_A = "GABC1234567890123456789012345678901234567890123456789012";
const ADDR_B = "GXYZ1234567890123456789012345678901234567890123456789012";

function makeEvent(overrides: Partial<ActivityEvent> = {}): ActivityEvent {
  return {
    id: "evt_1",
    type: "LISTED",
    listing_id: 1,
    title: "Test Artwork",
    price: "100",
    timestamp: 1_700_000_000_000,
    from: ADDR_A,
    to: ADDR_B,
    tx_hash: "abc123txhash",
    ...overrides,
  };
}

const defaultProps: ProvenanceTimelineProps = {
  events: [],
  isLoading: false,
  isLoadingMore: false,
  error: null,
  hasMore: false,
  onLoadMore: jest.fn(),
};

// ── Tests ─────────────────────────────────────────────────────────────────────

describe("ProvenanceTimeline", () => {
  describe("loading state", () => {
    it("renders a loading indicator when isLoading is true", () => {
      render(<ProvenanceTimeline {...defaultProps} isLoading />);
      expect(screen.getByTestId("timeline-loading")).toBeInTheDocument();
      expect(screen.queryByTestId("timeline-root")).not.toBeInTheDocument();
    });
  });

  describe("error state", () => {
    it("renders the error message when error is provided", () => {
      render(
        <ProvenanceTimeline {...defaultProps} error="Failed to fetch history" />
      );
      expect(screen.getByTestId("timeline-error")).toBeInTheDocument();
      expect(screen.getByText("Failed to fetch history")).toBeInTheDocument();
    });
  });

  describe("empty history", () => {
    it("renders the empty state when events is an empty array", () => {
      render(<ProvenanceTimeline {...defaultProps} events={[]} />);
      expect(screen.getByTestId("timeline-empty")).toBeInTheDocument();
      expect(
        screen.getByText(/no activity recorded yet/i)
      ).toBeInTheDocument();
    });
  });

  describe("event rendering", () => {
    const eventTypes: Array<[ActivityEvent["type"], string]> = [
      ["LISTED", "Created listing"],
      ["OFFER_SUBMITTED", "Submitted an offer"],
      ["OFFER_ACCEPTED", "Accepted an offer"],
      ["PURCHASE", "Purchased listing"],
      ["SALE", "Sold listing"],
      ["ROYALTY", "Royalty distributed"],
      ["CANCELLED", "Listing cancelled"],
      ["TRANSFER", "Transferred ownership"],
    ];

    it.each(eventTypes)(
      "renders %s event with label '%s'",
      (type, expectedLabel) => {
        render(
          <ProvenanceTimeline
            {...defaultProps}
            events={[makeEvent({ id: `evt_${type}`, type })]}
          />
        );
        expect(screen.getByText(expectedLabel)).toBeInTheDocument();
      }
    );

    it("renders multiple events", () => {
      const events: ActivityEvent[] = [
        makeEvent({ id: "e1", type: "LISTED" }),
        makeEvent({ id: "e2", type: "OFFER_SUBMITTED" }),
        makeEvent({ id: "e3", type: "PURCHASE" }),
      ];
      render(<ProvenanceTimeline {...defaultProps} events={events} />);
      expect(screen.getByText("Created listing")).toBeInTheDocument();
      expect(screen.getByText("Submitted an offer")).toBeInTheDocument();
      expect(screen.getByText("Purchased listing")).toBeInTheDocument();
    });

    it("renders events in the order provided (chronological)", () => {
      const events: ActivityEvent[] = [
        makeEvent({ id: "e1", type: "LISTED", timestamp: 1_000_000 }),
        makeEvent({ id: "e2", type: "OFFER_SUBMITTED", timestamp: 2_000_000 }),
        makeEvent({ id: "e3", type: "PURCHASE", timestamp: 3_000_000 }),
      ];
      render(<ProvenanceTimeline {...defaultProps} events={events} />);

      const items = screen.getAllByRole("listitem");
      expect(items[0]).toHaveAttribute(
        "data-testid",
        "timeline-event-LISTED"
      );
      expect(items[1]).toHaveAttribute(
        "data-testid",
        "timeline-event-OFFER_SUBMITTED"
      );
      expect(items[2]).toHaveAttribute(
        "data-testid",
        "timeline-event-PURCHASE"
      );
    });
  });

  describe("actor profile links", () => {
    it("renders a link to the from actor's profile page", () => {
      render(
        <ProvenanceTimeline
          {...defaultProps}
          events={[makeEvent({ from: ADDR_A, to: "—" })]}
        />
      );
      const links = screen.getAllByTestId("actor-link");
      expect(links[0]).toHaveAttribute("href", `/profile/${ADDR_A}`);
    });

    it("renders links for both from and to when they differ", () => {
      render(
        <ProvenanceTimeline
          {...defaultProps}
          events={[makeEvent({ from: ADDR_A, to: ADDR_B })]}
        />
      );
      const links = screen.getAllByTestId("actor-link");
      expect(links).toHaveLength(2);
      expect(links[0]).toHaveAttribute("href", `/profile/${ADDR_A}`);
      expect(links[1]).toHaveAttribute("href", `/profile/${ADDR_B}`);
    });

    it("renders '—' text when actor address is missing", () => {
      render(
        <ProvenanceTimeline
          {...defaultProps}
          events={[makeEvent({ from: "—", to: "—" })]}
        />
      );
      // No actor links rendered
      expect(screen.queryByTestId("actor-link")).not.toBeInTheDocument();
    });
  });

  describe("transaction explorer links", () => {
    it("renders an explorer link for a real tx hash", () => {
      render(
        <ProvenanceTimeline
          {...defaultProps}
          events={[makeEvent({ tx_hash: "realhash123" })]}
        />
      );
      const txLink = screen.getByTestId("tx-link");
      expect(txLink).toHaveAttribute(
        "href",
        "https://horizon-testnet.stellar.org/transactions/realhash123"
      );
      expect(txLink).toHaveAttribute("target", "_blank");
      expect(txLink).toHaveAttribute("rel", "noopener noreferrer");
    });

    it("does not render an explorer link for ledger_ placeholder hashes", () => {
      render(
        <ProvenanceTimeline
          {...defaultProps}
          events={[makeEvent({ tx_hash: "ledger_123456" })]}
        />
      );
      expect(screen.queryByTestId("tx-link")).not.toBeInTheDocument();
    });
  });

  describe("pagination", () => {
    it("renders 'Load more' button when hasMore is true", () => {
      render(
        <ProvenanceTimeline
          {...defaultProps}
          events={[makeEvent()]}
          hasMore
        />
      );
      expect(screen.getByTestId("load-more-button")).toBeInTheDocument();
    });

    it("does not render 'Load more' when hasMore is false", () => {
      render(
        <ProvenanceTimeline
          {...defaultProps}
          events={[makeEvent()]}
          hasMore={false}
        />
      );
      expect(screen.queryByTestId("load-more-button")).not.toBeInTheDocument();
    });

    it("calls onLoadMore when the button is clicked", () => {
      const onLoadMore = jest.fn();
      render(
        <ProvenanceTimeline
          {...defaultProps}
          events={[makeEvent()]}
          hasMore
          onLoadMore={onLoadMore}
        />
      );
      fireEvent.click(screen.getByTestId("load-more-button"));
      expect(onLoadMore).toHaveBeenCalledTimes(1);
    });

    it("disables the button and shows loading text while isLoadingMore", () => {
      render(
        <ProvenanceTimeline
          {...defaultProps}
          events={[makeEvent()]}
          hasMore
          isLoadingMore
        />
      );
      const btn = screen.getByTestId("load-more-button");
      expect(btn).toBeDisabled();
      expect(btn).toHaveTextContent(/loading/i);
    });
  });
});
