/**
 * ProvenanceTimeline — on-chain history timeline for a listing.
 *
 * Renders events chronologically (oldest → newest) with:
 *  - Event icon and human-readable action label
 *  - Actor address linked to their profile page
 *  - Transaction hash linked to the blockchain explorer
 *  - Relative and absolute timestamps
 *  - "Load more" pagination
 *
 * Supported event types:
 *   LISTED, OFFER_SUBMITTED, OFFER_ACCEPTED, PURCHASE, SALE,
 *   ROYALTY, CANCELLED, TRANSFER
 *
 * Pagination:
 *   The component fetches 20 events per page. A "Load more" button
 *   appears when additional pages are available from the indexer.
 */

import React from "react";
import Link from "next/link";
import {
  Tag,
  ShoppingCart,
  HandCoins,
  CheckCheck,
  ArrowRightLeft,
  TrendingUp,
  XCircle,
  History,
  ExternalLink,
  Loader2,
} from "lucide-react";
import { ActivityEvent } from "@/lib/indexer";
import { config } from "@/lib/config";

// ── Explorer URL builder ───────────────────────────────────────────────────────

function explorerTxUrl(txHash: string): string | null {
  if (!txHash || txHash.startsWith("ledger_")) return null;
  const base =
    config.network === "mainnet"
      ? "https://horizon.stellar.org/transactions"
      : "https://horizon-testnet.stellar.org/transactions";
  return `${base}/${txHash}`;
}

// ── Event metadata ─────────────────────────────────────────────────────────────

interface EventMeta {
  label: string;
  icon: React.ReactNode;
  /** Tailwind classes for the icon bubble */
  iconBg: string;
  iconColor: string;
}

function getEventMeta(type: ActivityEvent["type"]): EventMeta {
  switch (type) {
    case "LISTED":
      return {
        label: "Created listing",
        icon: <Tag size={14} />,
        iconBg: "bg-brand-500/20",
        iconColor: "text-brand-400",
      };
    case "OFFER_SUBMITTED":
      return {
        label: "Submitted an offer",
        icon: <HandCoins size={14} />,
        iconBg: "bg-white/10",
        iconColor: "text-white/70",
      };
    case "OFFER_ACCEPTED":
      return {
        label: "Accepted an offer",
        icon: <CheckCheck size={14} />,
        iconBg: "bg-mint-500/20",
        iconColor: "text-mint-400",
      };
    case "PURCHASE":
      return {
        label: "Purchased listing",
        icon: <ShoppingCart size={14} />,
        iconBg: "bg-mint-500/20",
        iconColor: "text-mint-400",
      };
    case "SALE":
      return {
        label: "Sold listing",
        icon: <ShoppingCart size={14} />,
        iconBg: "bg-mint-500/20",
        iconColor: "text-mint-400",
      };
    case "ROYALTY":
      return {
        label: "Royalty distributed",
        icon: <TrendingUp size={14} />,
        iconBg: "bg-brand-500/10",
        iconColor: "text-brand-300",
      };
    case "CANCELLED":
      return {
        label: "Listing cancelled",
        icon: <XCircle size={14} />,
        iconBg: "bg-terracotta-500/20",
        iconColor: "text-terracotta-400",
      };
    case "TRANSFER":
      return {
        label: "Transferred ownership",
        icon: <ArrowRightLeft size={14} />,
        iconBg: "bg-brand-500/20",
        iconColor: "text-brand-400",
      };
    default:
      return {
        label: type,
        icon: <History size={14} />,
        iconBg: "bg-white/10",
        iconColor: "text-white/50",
      };
  }
}

// ── Helpers ────────────────────────────────────────────────────────────────────

function shortAddr(addr: string): string {
  if (!addr || addr === "—") return "—";
  if (addr.length <= 12) return addr;
  return `${addr.slice(0, 6)}…${addr.slice(-4)}`;
}

function formatTimestamp(ts: number): string {
  return new Date(ts).toLocaleString(undefined, {
    dateStyle: "medium",
    timeStyle: "short",
  });
}

// ── Sub-components ─────────────────────────────────────────────────────────────

function ActorLink({ address }: { address: string }) {
  if (!address || address === "—") {
    return <span className="text-white/40">—</span>;
  }
  return (
    <Link
      href={`/profile/${address}`}
      className="font-mono text-brand-400 hover:text-brand-300 hover:underline transition-colors"
      data-testid="actor-link"
    >
      {shortAddr(address)}
    </Link>
  );
}

function TxLink({ txHash }: { txHash: string }) {
  const url = explorerTxUrl(txHash);
  if (!url) return null;
  return (
    <a
      href={url}
      target="_blank"
      rel="noopener noreferrer"
      className="inline-flex items-center gap-1 text-[10px] font-mono text-white/30 hover:text-brand-400 transition-colors"
      data-testid="tx-link"
    >
      {shortAddr(txHash)}
      <ExternalLink size={9} />
    </a>
  );
}

// ── Main component ─────────────────────────────────────────────────────────────

export interface ProvenanceTimelineProps {
  events: ActivityEvent[];
  isLoading: boolean;
  isLoadingMore: boolean;
  error: string | null;
  hasMore: boolean;
  onLoadMore: () => void;
}

export function ProvenanceTimeline({
  events,
  isLoading,
  isLoadingMore,
  error,
  hasMore,
  onLoadMore,
}: ProvenanceTimelineProps) {
  if (isLoading) {
    return (
      <div
        className="flex flex-col items-center justify-center py-12 gap-3"
        data-testid="timeline-loading"
      >
        <Loader2 size={28} className="animate-spin text-brand-400" />
        <p className="text-xs text-white/40 italic">Loading provenance…</p>
      </div>
    );
  }

  if (error) {
    return (
      <div
        className="py-8 text-center"
        data-testid="timeline-error"
      >
        <p className="text-terracotta-400 text-sm font-medium">{error}</p>
      </div>
    );
  }

  if (events.length === 0) {
    return (
      <div
        className="py-10 text-center text-white/30"
        data-testid="timeline-empty"
      >
        <History size={40} className="mx-auto mb-4 opacity-20" />
        <p className="italic text-sm">No activity recorded yet</p>
      </div>
    );
  }

  return (
    <div data-testid="timeline-root">
      <ol className="space-y-0" aria-label="Provenance history">
        {events.map((evt, idx) => {
          const meta = getEventMeta(evt.type);
          const isLast = idx === events.length - 1 && !hasMore;

          return (
            <li
              key={evt.id}
              className="flex gap-4 relative"
              data-testid={`timeline-event-${evt.type}`}
            >
              {/* Vertical connector line */}
              {!isLast && (
                <div
                  aria-hidden="true"
                  className="absolute left-[15px] top-8 bottom-0 w-px bg-white/10"
                />
              )}

              {/* Icon bubble */}
              <div
                aria-hidden="true"
                className={`w-8 h-8 rounded-full flex items-center justify-center shrink-0 z-10 mt-0.5 ${meta.iconBg} ${meta.iconColor}`}
              >
                {meta.icon}
              </div>

              {/* Content */}
              <div className="flex-1 pb-6 min-w-0">
                <div className="flex flex-wrap items-center justify-between gap-2 mb-1">
                  <span className="text-xs font-bold uppercase tracking-widest text-white">
                    {meta.label}
                  </span>
                  <span className="text-[10px] text-white/30 font-mono shrink-0">
                    {formatTimestamp(evt.timestamp)}
                  </span>
                </div>

                {/* Actor row */}
                <p className="text-xs text-white/50 mb-1.5 truncate">
                  <ActorLink address={evt.from} />
                  {evt.to && evt.to !== "—" && evt.to !== evt.from && (
                    <>
                      {" → "}
                      <ActorLink address={evt.to} />
                    </>
                  )}
                </p>

                <div className="flex items-center justify-between gap-2">
                  {Number(evt.price) > 0 && (
                    <span className="text-xs font-bold text-brand-400">
                      {evt.price} XLM
                    </span>
                  )}
                  <TxLink txHash={evt.tx_hash} />
                </div>
              </div>
            </li>
          );
        })}
      </ol>

      {/* Load more */}
      {hasMore && (
        <div className="pt-2 flex justify-center">
          <button
            onClick={onLoadMore}
            disabled={isLoadingMore}
            className="flex items-center gap-2 text-xs font-bold text-brand-400 hover:text-brand-300 disabled:opacity-50 transition-colors"
            data-testid="load-more-button"
          >
            {isLoadingMore ? (
              <Loader2 size={12} className="animate-spin" />
            ) : null}
            {isLoadingMore ? "Loading…" : "Load more"}
          </button>
        </div>
      )}
    </div>
  );
}
