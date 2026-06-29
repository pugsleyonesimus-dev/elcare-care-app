// ─────────────────────────────────────────────────────────────
// app/dashboard/page.tsx — Artist Dashboard
// ─────────────────────────────────────────────────────────────

"use client";

import { useState, useEffect } from "react";
import { useWalletContext } from "@/context/WalletContext";
import { useArtistListings, useCancelListing } from "@/hooks/useMarketplace";
import { ListingForm } from "@/components/ListingForm";
import { AuctionForm } from "@/components/AuctionForm";
import { stroopsToXlm, Listing } from "@/lib/contract";
import { fetchArtistMetrics, ArtistMetrics, MetricsRange } from "@/lib/indexer";
import { Plus, Package, XCircle, Wallet, Edit2, Activity, TrendingUp, Gavel, BarChart2 } from "lucide-react";
import { WalletGuard } from "@/components/WalletGuard";
import { SUPPORTED_TOKENS } from "@/config/tokens";
import { clsx } from "clsx";

type Tab = "listings" | "list" | "edit" | "auction" | "metrics";

const STATUS_COLOR: Record<string, string> = {
  Active: "text-green-600 bg-green-50",
  Sold: "text-gray-500 bg-gray-100",
  Cancelled: "text-red-500 bg-red-50",
};

// ── Inline bar chart (no extra deps) ─────────────────────────────────────────

function MiniBarChart({ data, label }: { data: { date: string; count: number }[]; label: string }) {
  if (data.length === 0) {
    return (
      <div className="flex items-center justify-center h-24 text-white/30 text-xs">
        No {label} data yet
      </div>
    );
  }
  const max = Math.max(...data.map((d) => d.count), 1);
  return (
    <div className="w-full">
      <p className="text-[10px] uppercase tracking-widest font-bold text-white/40 mb-3">{label}</p>
      <div className="flex items-end gap-1 h-20">
        {data.map((d) => (
          <div key={d.date} className="flex-1 flex flex-col items-center gap-1 group" title={`${d.date}: ${d.count}`}>
            <div
              className="w-full rounded-t bg-brand-500/60 group-hover:bg-brand-400 transition-all"
              style={{ height: `${Math.max((d.count / max) * 100, 8)}%` }}
            />
          </div>
        ))}
      </div>
      <div className="flex justify-between mt-1">
        <span className="text-[9px] text-white/20">{data[0]?.date}</span>
        <span className="text-[9px] text-white/20">{data[data.length - 1]?.date}</span>
      </div>
    </div>
  );
}

// ── MetricsDashboard ──────────────────────────────────────────────────────────

function MetricsDashboard({ publicKey }: { publicKey: string }) {
  const [range, setRange] = useState<MetricsRange>("week");
  const [metrics, setMetrics] = useState<ArtistMetrics | null>(null);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    if (!publicKey) return;
    setLoading(true);
    fetchArtistMetrics(publicKey, range)
      .then(setMetrics)
      .finally(() => setLoading(false));
  }, [publicKey, range]);

  const ranges: MetricsRange[] = ["day", "week", "month", "all"];

  return (
    <div className="space-y-8">
      {/* Range selector */}
      <div className="flex items-center gap-2">
        {ranges.map((r) => (
          <button
            key={r}
            onClick={() => setRange(r)}
            className={clsx(
              "px-4 py-1.5 rounded-full text-[11px] font-bold uppercase tracking-widest border transition-all",
              range === r
                ? "bg-brand-500/20 text-brand-400 border-brand-500/40"
                : "bg-white/5 text-white/40 border-white/10 hover:text-white hover:border-white/20"
            )}
          >
            {r}
          </button>
        ))}
      </div>

      {loading ? (
        <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
          {[1, 2, 3, 4].map((i) => (
            <div key={i} className="h-24 animate-pulse rounded-[2rem] bg-white/[0.03] border border-white/5" />
          ))}
        </div>
      ) : metrics ? (
        <>
          {/* KPI cards */}
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            {[
              { label: "Listings", value: metrics.totalListings, color: "brand" },
              { label: "Sales", value: metrics.totalSales, color: "mint" },
              { label: "Unique Buyers", value: metrics.uniqueBuyers, color: "terracotta" },
              {
                label: "Conversion",
                value: `${(metrics.conversionRate * 100).toFixed(1)}%`,
                color: "brand",
              },
            ].map(({ label, value, color }) => (
              <div
                key={label}
                className="rounded-[2rem] bg-white/[0.03] border border-white/5 p-5 space-y-2"
              >
                <p className="text-[10px] uppercase tracking-widest font-bold text-white/40">{label}</p>
                <p
                  className={clsx(
                    "text-3xl font-display font-bold",
                    color === "brand" ? "text-brand-400" :
                      color === "mint" ? "text-mint-400" : "text-terracotta-400"
                  )}
                >
                  {value}
                </p>
              </div>
            ))}
          </div>

          {/* Volume */}
          <div className="rounded-[2rem] bg-white/[0.03] border border-white/5 p-6 space-y-2">
            <p className="text-[10px] uppercase tracking-widest font-bold text-white/40">Total Volume</p>
            <p className="text-4xl font-display font-bold text-white">
              {stroopsToXlm(BigInt(Math.round(parseFloat(metrics.totalVolume))))}
              <span className="ml-2 text-sm font-normal text-brand-400">XLM</span>
            </p>
          </div>

          {/* Sales timeline chart */}
          <div className="rounded-[2rem] bg-white/[0.03] border border-white/5 p-6">
            <MiniBarChart data={metrics.salesTimeline} label="Sales over time" />
          </div>
        </>
      ) : (
        <div className="flex items-center justify-center h-40 text-white/30 text-sm">
          No metrics available
        </div>
      )}
    </div>
  );
}

export default function DashboardPage() {
  const { publicKey } = useWalletContext();
  const { listings, isLoading, refresh } = useArtistListings(publicKey);
  const { cancel, isCancelling } = useCancelListing(publicKey);
  const [tab, setTab] = useState<Tab>("listings");
  const [editingListing, setEditingListing] = useState<Listing | null>(null);  const activeCnt = listings.filter((l: Listing) => l.status === "Active").length;
  const soldCnt = listings.filter((l: Listing) => l.status === "Sold").length;

  const getTokenSymbol = (address: string) => {
    return SUPPORTED_TOKENS.find(t => t.address === address)?.symbol || "Tokens";
  };

  return (
    <div className="min-h-screen bg-midnight-950 pb-20 pt-24 selection:bg-brand-500 selection:text-white">
      <div className="fixed inset-0 pointer-events-none opacity-[0.03] z-0 overflow-hidden">
        <div className="absolute inset-0 tribal-pattern scale-150 rotate-12" />
      </div>

      <WalletGuard actionName="To access your artist dashboard">
        <div className="relative z-10 mx-auto max-w-7xl px-4 sm:px-6 lg:px-8">

          <div className="relative mb-12 overflow-hidden rounded-[3rem] bg-midnight-900 border border-white/5 shadow-2xl p-8 sm:p-12">
            <div className="absolute -top-24 -right-24 h-64 w-64 rounded-full bg-brand-500/10 blur-[100px]" />
            <div className="absolute -bottom-24 -left-24 h-64 w-64 rounded-full bg-mint-500/10 blur-[100px]" />
            <div className="absolute top-0 right-0 left-0 tribal-strip h-1.5 opacity-40" />

            <div className="relative flex flex-col items-center justify-between gap-10 md:flex-row md:items-start">
              <div className="flex flex-col items-center gap-8 md:flex-row md:items-start text-center md:text-left">
                <div className="relative group">
                  <div className="absolute -inset-1.5 rounded-[2.5rem] bg-gradient-to-tr from-brand-500 via-terracotta-400 to-mint-500 opacity-80 blur transition duration-700 group-hover:opacity-100 group-hover:duration-200" />
                  <div className="relative flex h-28 w-28 items-center justify-center rounded-[2.2rem] bg-midnight-950 border border-white/10 shadow-2xl overflow-hidden group-hover:scale-[1.02] transition-transform duration-500">
                    <Wallet size={56} className="text-brand-400/80 group-hover:text-brand-400 transition-colors" />
                  </div>
                </div>

                <div className="flex flex-col gap-4">
                  <div className="space-y-1">
                    <h1 className="font-display text-4xl sm:text-5xl font-bold tracking-tight text-white">
                      Artist <span className="text-brand-400">Dashboard</span>
                    </h1>
                    <p className="text-brand-300/60 font-medium text-sm tracking-widest uppercase">Manage your art collection</p>
                  </div>

                  <div className="flex flex-col gap-3 font-mono">
                    <p className="text-[11px] sm:text-xs text-mint-400/90 break-all bg-white/5 px-4 py-2.5 rounded-2xl border border-white/10 backdrop-blur-md shadow-inner inline-flex">
                      {publicKey}
                    </p>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <div className="mb-12 grid gap-6 sm:grid-cols-3">
            {[
              { label: "Total Artworks", value: listings.length, icon: Package, color: "brand" },
              { label: "Available Now", value: activeCnt, icon: Activity, color: "mint" },
              { label: "Successful Sales", value: soldCnt, icon: TrendingUp, color: "terracotta" },
            ].map(({ label, value, icon: Icon, color }) => (
              <div
                key={label}
                className={clsx(
                  "group relative rounded-[2.5rem] bg-white/5 border border-white/10 p-6 backdrop-blur-md transition-all duration-500 hover:border-white/20 overflow-hidden shadow-2xl",
                  color === "brand" && "hover:border-brand-500/30 hover:bg-white/[0.07]",
                  color === "mint" && "hover:border-mint-500/30 hover:bg-white/[0.07]",
                  color === "terracotta" && "hover:border-terracotta-500/30 hover:bg-white/[0.07]"
                )}
              >
                <div className={clsx(
                  "absolute top-0 right-0 w-32 h-32 rounded-full blur-3xl transition-colors",
                  color === "brand" && "bg-brand-500/5 group-hover:bg-brand-500/10",
                  color === "mint" && "bg-mint-500/5 group-hover:bg-mint-500/10",
                  color === "terracotta" && "bg-terracotta-500/5 group-hover:bg-terracotta-500/10"
                )} />
                <div className="flex items-center justify-between relative z-10">
                  <p className="text-[10px] uppercase tracking-[0.3em] font-bold text-white/40">{label}</p>
                  <div className={clsx(
                    "rounded-full p-2 border",
                    color === "brand" ? "border-brand-500/20 bg-brand-500/10" :
                      color === "mint" ? "border-mint-500/20 bg-mint-500/10" :
                        "border-terracotta-500/20 bg-terracotta-500/10"
                  )}>
                    <Icon size={16} className={clsx(
                      color === "brand" ? "text-brand-400" :
                        color === "mint" ? "text-mint-400" :
                          "text-terracotta-400"
                    )} />
                  </div>
                </div>
                <p className="mt-4 text-4xl font-display font-bold tracking-tight text-white relative z-10">{value}</p>
              </div>
            ))}
          </div>

          <div className="mb-10 flex flex-wrap gap-2 border-b border-white/5 pb-px overflow-x-auto no-scrollbar scroll-smooth">
            <button
              onClick={() => setTab("listings")}
              className={clsx(
                "group relative flex items-center gap-3 px-6 sm:px-8 py-5 text-sm font-bold transition-all duration-500 whitespace-nowrap",
                tab === "listings" ? "text-brand-400" : "text-white/40 hover:text-white"
              )}
            >
              <Package size={18} className={clsx("transition-all duration-500 group-hover:scale-125", tab === "listings" && "text-brand-400 drop-shadow-[0_0_8px_rgba(226,125,96,0.5)]")} />
              Gallery
              {tab === "listings" && (
                <div className="absolute inset-x-4 bottom-0 h-1.5 rounded-t-full bg-brand-500 shadow-[0_-5px_15px_rgba(226,125,96,0.6)] animate-slide-in-right" />
              )}
            </button>
            <button
              onClick={() => setTab("metrics")}
              className={clsx(
                "group relative flex items-center gap-3 px-6 sm:px-8 py-5 text-sm font-bold transition-all duration-500 whitespace-nowrap",
                tab === "metrics" ? "text-mint-400" : "text-white/40 hover:text-white"
              )}
            >
              <BarChart2 size={18} className={clsx("transition-all duration-500 group-hover:scale-125", tab === "metrics" && "text-mint-400 drop-shadow-[0_0_8px_rgba(38,167,110,0.5)]")} />
              Metrics
              {tab === "metrics" && (
                <div className="absolute inset-x-4 bottom-0 h-1.5 rounded-t-full bg-mint-500 shadow-[0_-5px_15px_rgba(38,167,110,0.6)] animate-slide-in-right" />
              )}
            </button>
            <button
              onClick={() => setTab("list")}
              className={clsx(
                "group relative flex items-center gap-3 px-6 sm:px-8 py-5 text-sm font-bold transition-all duration-500 whitespace-nowrap",
                tab === "list" ? "text-mint-400" : "text-white/40 hover:text-white"
              )}
            >
              <Plus size={18} className={clsx("transition-all duration-500 group-hover:scale-125", tab === "list" && "text-mint-400 drop-shadow-[0_0_8px_rgba(38,167,110,0.5)]")} />
              New Listing
              {tab === "list" && (
                <div className="absolute inset-x-4 bottom-0 h-1.5 rounded-t-full bg-mint-500 shadow-[0_-5px_15px_rgba(38,167,110,0.6)] animate-slide-in-right" />
              )}
            </button>
            <button
              onClick={() => setTab("auction")}
              className={clsx(
                "group relative flex items-center gap-3 px-6 sm:px-8 py-5 text-sm font-bold transition-all duration-500 whitespace-nowrap",
                tab === "auction" ? "text-brand-400" : "text-white/40 hover:text-white"
              )}
            >
              <Gavel size={18} className={clsx("transition-all duration-500 group-hover:scale-125", tab === "auction" && "text-brand-400 drop-shadow-[0_0_8px_rgba(226,125,96,0.5)]")} />
              New Auction
              {tab === "auction" && (
                <div className="absolute inset-x-4 bottom-0 h-1.5 rounded-t-full bg-brand-500 shadow-[0_-5px_15px_rgba(226,125,96,0.6)] animate-slide-in-right" />
              )}
            </button>
            {tab === "edit" && (
              <button
                className="group relative flex items-center gap-3 px-6 sm:px-8 py-5 text-sm font-bold transition-all duration-500 whitespace-nowrap text-terracotta-400"
              >
                <Edit2 size={18} className="text-terracotta-400 drop-shadow-[0_0_8px_rgba(235,79,27,0.5)]" />
                Edit Listing #{editingListing?.listing_id}
                <div className="absolute inset-x-4 bottom-0 h-1.5 rounded-t-full bg-terracotta-500 shadow-[0_-5px_15px_rgba(235,79,27,0.6)] animate-slide-in-right" />
              </button>
            )}
          </div>

          <div className="animate-fade-in duration-700">
            {tab === "metrics" ? (
              <MetricsDashboard publicKey={publicKey ?? ""} />
            ) : tab === "list" ? (
              <div className="w-full">
                <ListingForm
                  onSuccess={() => {
                    refresh();
                    setTab("listings");
                  }}
                  onCancel={() => setTab("listings")}
                />
              </div>
            ) : tab === "auction" ? (
              <div className="w-full">
                <AuctionForm
                  onSuccess={() => setTab("listings")}
                  onCancel={() => setTab("listings")}
                />
              </div>
            ) : tab === "edit" ? (
              <div className="w-full">
                {editingListing && (
                  <ListingForm
                    listing={editingListing}
                    onSuccess={() => {
                      refresh();
                      setTab("listings");
                      setEditingListing(null);
                    }}
                    onCancel={() => {
                      setTab("listings");
                      setEditingListing(null);
                    }}
                  />
                )}
              </div>
            ) : (
              <>
                {isLoading ? (
                  <div className="space-y-4">
                    {[1, 2, 3].map((i) => (
                      <div key={i} className="h-24 animate-pulse rounded-[2rem] bg-white/[0.03] border border-white/5" />
                    ))}
                  </div>
                ) : listings.length === 0 ? (
                  <div className="col-span-full flex flex-col items-center justify-center rounded-[3.5rem] bg-midnight-900/50 border-2 border-dashed border-white/5 py-32 px-10 text-center backdrop-blur-sm relative overflow-hidden group">
                    <div className="absolute inset-0 tribal-pattern opacity-[0.02] group-hover:opacity-[0.04] transition-opacity duration-500" />
                    <div className="relative mb-10 flex h-28 w-28 items-center justify-center rounded-[2.5rem] bg-midnight-950 text-white/10 shadow-inner group-hover:text-brand-500/30 transition-colors duration-500">
                      <Package size={48} />
                    </div>
                    <h3 className="font-display text-3xl font-bold text-white tracking-tight relative z-10">No listings yet.</h3>
                    <p className="mt-4 max-w-sm text-sm text-brand-300/40 leading-relaxed font-medium relative z-10">Start your journey by creating your first listing.</p>
                    <button
                      onClick={() => setTab("list")}
                      className="mt-8 rounded-2xl bg-brand-500 px-8 py-3.5 text-lg font-bold text-white hover:bg-brand-600 shadow-xl shadow-brand-500/20 transition-all hover:scale-[1.02] relative z-10"
                    >
                      Create your first listing
                    </button>
                  </div>
                ) : (
                  <div className="grid gap-6">
                    {listings.map((l: Listing) => (
                      <div key={l.listing_id} className="group relative flex flex-col sm:flex-row items-center justify-between gap-6 rounded-[2.5rem] bg-white/[0.03] hover:bg-white/[0.07] hover:border-white/10 transition-all duration-500 border border-white/5 p-6 shadow-2xl">
                        <div className="flex items-center gap-6 w-full sm:w-auto">
                          <div className="h-16 w-16 rounded-[1.2rem] bg-brand-500/10 flex items-center justify-center text-brand-400 border border-brand-500/20 shadow-inner">
                            <span className="font-bold text-xl">#{l.listing_id}</span>
                          </div>
                          <div className="flex flex-col gap-1.5">
                            <span className="text-[10px] font-mono text-white/40 tracking-wider">CID: {l.metadata_cid?.slice(0, 16) || "N/A"}…</span>
                            <div className="flex items-center gap-3">
                              <span className="font-display text-2xl font-bold text-white">{stroopsToXlm(l.price)}</span>
                              <span className="text-[10px] font-bold text-brand-400 uppercase tracking-widest">{getTokenSymbol(l.token)}</span>
                            </div>
                          </div>
                        </div>

                        <div className="flex flex-col sm:flex-row items-start sm:items-center justify-between w-full sm:w-auto gap-6 sm:gap-8 border-t sm:border-t-0 border-white/5 pt-4 sm:pt-0">
                          <div className="flex items-center gap-2">
                            <span className={clsx(
                              "px-4 py-1.5 rounded-full text-[10px] font-bold uppercase tracking-[0.2em] border",
                              l.status === "Active" ? "bg-mint-500/10 text-mint-400 border-mint-500/20" :
                                l.status === "Sold" ? "bg-white/5 text-white/40 border-white/10" :
                                  "bg-terracotta-500/10 text-terracotta-400 border-terracotta-500/20"
                            )}>
                              {l.status}
                            </span>
                          </div>

                          <div className="flex items-center gap-3 w-full sm:w-auto sm:opacity-0 group-hover:opacity-100 transition-opacity justify-end">
                            {l.status === "Active" && (
                              <>
                                <button
                                  onClick={() => {
                                    setEditingListing(l);
                                    setTab("edit");
                                  }}
                                  className="flex flex-1 sm:flex-none justify-center items-center gap-2 rounded-xl bg-white/5 hover:bg-brand-500/20 px-4 py-2 text-sm font-bold text-white hover:text-brand-400 border border-white/10 hover:border-brand-500/30 transition-all shadow-sm"
                                >
                                  <Edit2 size={16} />
                                  Edit
                                </button>
                                <button
                                  onClick={async () => {
                                    await cancel(l.listing_id);
                                    refresh();
                                  }}
                                  disabled={isCancelling}
                                  className="flex flex-1 sm:flex-none justify-center items-center gap-2 rounded-xl bg-white/5 hover:bg-terracotta-500/20 px-4 py-2 text-sm font-bold text-terracotta-400 border border-white/10 hover:border-terracotta-500/30 transition-all shadow-sm disabled:opacity-50"
                                >
                                  <XCircle size={16} />
                                  Cancel
                                </button>
                              </>
                            )}
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                )}
              </>
            )}
          </div>
        </div>
      </WalletGuard>
    </div>
  );
}

