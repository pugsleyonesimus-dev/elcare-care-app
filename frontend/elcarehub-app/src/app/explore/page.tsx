// ─────────────────────────────────────────────────────────────
// app/explore/page.tsx — Browse / Explore All Listings
//
// Full catalogue page with search, filtering, sorting, and
// pagination for discovering marketplace listings at scale.
// ─────────────────────────────────────────────────────────────

"use client";

import { useState, useMemo, useCallback, useEffect, useRef } from "react";
import { Listing, stroopsToXlm } from "@/lib/contract";
import { ListingCard } from "@/components/ListingCard";
import { ListingCardSkeleton } from "@/components/Skeletons";
import {
  ChevronLeft,
  ChevronRight,
} from "lucide-react";
import { SearchFilter, Filters, SortOption } from "@/components/SearchFilter";
import { fetchMetadata, ArtworkMetadata } from "@/lib/ipfs";
import { fetchListings } from "@/lib/indexer";
import { getAllListings } from "@/lib/contract";
import { useFilterUrlSync } from "@/hooks/useFilterUrlSync";

// ── Types ────────────────────────────────────────────────────

const PAGE_SIZE = 12;

// ── Metadata cache for category / text search ────────────────

const metadataCache = new Map<string, ArtworkMetadata | null>();

async function getCachedMetadata(cid?: string): Promise<ArtworkMetadata | null> {
  if (!cid) return null;
  if (metadataCache.has(cid)) return metadataCache.get(cid) ?? null;
  try {
    const meta = await fetchMetadata(cid);
    metadataCache.set(cid, meta);
    return meta;
  } catch {
    metadataCache.set(cid, null);
    return null;
  }
}

// ── Page Component ───────────────────────────────────────────

export default function ExplorePage() {
  const [allListings, setAllListings] = useState<Listing[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // ── URL-synced filters (ISSUE-100) ─────────────────────
  const { initialFilters, initialPage, syncToUrl } = useFilterUrlSync();

  const [filters, setFilters] = useState<Filters>(initialFilters);
  const [page, setPage] = useState(initialPage);
  const [showFilters, setShowFilters] = useState(false);

  const [metadataMap, setMetadataMap] = useState<Map<string, ArtworkMetadata | null>>(new Map());

  // Debounce search so we don't fire on every keystroke
  const [debouncedSearch, setDebouncedSearch] = useState(initialFilters.search);
  const searchTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    if (searchTimer.current) clearTimeout(searchTimer.current);
    searchTimer.current = setTimeout(() => setDebouncedSearch(filters.search), 350);
    return () => { if (searchTimer.current) clearTimeout(searchTimer.current); };
  }, [filters.search]);

  // ── Debounced indexer fetch (ISSUE-100) ────────────────
  // We debounce the entire load so rapid changes to any filter
  // (status, price, search) only trigger a single indexer call.
  const load = useCallback(async () => {
    setIsLoading(true);
    setError(null);
    try {
      const opts: Parameters<typeof fetchListings>[0] = { limit: 1000 };
      if (filters.status !== "All") opts.status = filters.status;
      if (filters.minPrice) opts.minPrice = filters.minPrice;
      if (filters.maxPrice) opts.maxPrice = filters.maxPrice;
      if (debouncedSearch.trim()) opts.search = debouncedSearch.trim();

      const res = await fetchListings(opts);
      const rows = Array.isArray(res.listings) ? (res.listings as Listing[]) : [];
      if (rows.length > 0) {
        setAllListings(rows);
      } else {
        // Fallback to on-chain scan when indexer returns nothing
        const all = await getAllListings();
        setAllListings(all);
      }
    } catch {
      try {
        const all = await getAllListings();
        setAllListings(all);
      } catch (e: unknown) {
        setError(e instanceof Error ? e.message : "Failed to load listings");
      }
    } finally {
      setIsLoading(false);
    }
  }, [filters.status, filters.minPrice, filters.maxPrice, debouncedSearch]);

  // Debounced load effect — 350ms debounce window so rapid
  // filter changes produce a single indexer request.
  const debouncedLoadRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => {
    if (debouncedLoadRef.current) clearTimeout(debouncedLoadRef.current);
    debouncedLoadRef.current = setTimeout(load, 350);
    return () => {
      if (debouncedLoadRef.current) clearTimeout(debouncedLoadRef.current);
    };
  }, [load]);

  // ── Sync filters & page to URL (ISSUE-100) ───────────
  useEffect(() => {
    syncToUrl(filters, page);
  }, [filters, page, syncToUrl]);

  // Resolve metadata for category / full-text search (client-side only)
  useEffect(() => {
    if (allListings.length === 0) return;
    let cancelled = false;
    const resolveAll = async () => {
      const entries: [string, ArtworkMetadata | null][] = [];
      await Promise.all(
        allListings.map(async (l) => {
          if (!l.metadata_cid) return;
          const meta = await getCachedMetadata(l.metadata_cid);
          entries.push([l.metadata_cid, meta]);
        })
      );
      if (!cancelled) setMetadataMap(new Map(entries));
    };
    resolveAll();
    return () => { cancelled = true; };
  }, [allListings]);

  // ── Client-side post-filter for category + sort ───────────

  const filtered = useMemo(() => {
    let result = [...allListings];

    // Category filter (IPFS metadata — client-side only)
    if (filters.category !== "All") {
      result = result.filter((l) => {
        const meta = l.metadata_cid ? metadataMap.get(l.metadata_cid) : null;
        return meta?.category === filters.category;
      });
    }

    // Sort
    switch (filters.sort) {
      case "newest":
        result.sort((a, b) => b.created_at - a.created_at);
        break;
      case "oldest":
        result.sort((a, b) => a.created_at - b.created_at);
        break;
      case "price-low":
        result.sort((a, b) => Number(a.price - b.price));
        break;
      case "price-high":
        result.sort((a, b) => Number(b.price - a.price));
        break;
    }

    return result;
  }, [allListings, filters.category, filters.sort, metadataMap]);

  // ── Pagination ───────────────────────────────────────────

  const totalPages = Math.max(1, Math.ceil(filtered.length / PAGE_SIZE));
  const paginatedListings = useMemo(() => {
    const start = (page - 1) * PAGE_SIZE;
    return filtered.slice(start, start + PAGE_SIZE);
  }, [filtered, page]);

  const goToPage = useCallback(
    (p: number) => {
      setPage(Math.max(1, Math.min(p, totalPages)));
      window.scrollTo({ top: 0, behavior: "smooth" });
    },
    [totalPages]
  );

  // ── Stats ────────────────────────────────────────────────

  const activeCnt = allListings.filter((l) => l.status === "Active").length;
  const soldCnt = allListings.filter((l) => l.status === "Sold").length;

  const hasActiveFilters = filters.search !== "" || filters.status !== "All" || filters.category !== "All" || filters.minPrice !== "" || filters.maxPrice !== "";

  return (
    <div className="min-h-screen bg-gray-50">
      {/* Header */}
      <div className="bg-midnight-900 pt-32 pb-16">
        <div className="mx-auto max-w-7xl px-4 sm:px-6">
          <div className="flex flex-col md:flex-row md:items-end md:justify-between gap-8">
            <div className="space-y-4">
              <h1 className="text-5xl font-display font-bold text-white tracking-tight">
                Explore Artworks
              </h1>
              <p className="max-w-xl text-xl text-white/60 font-inter leading-relaxed">
                Discover and collect unique African art on the blockchain
              </p>
            </div>

            {/* Stats */}
            <div className="flex flex-wrap gap-8 md:gap-12">
              {[
                { label: "Total Art", value: allListings.length },
                { label: "Active", value: activeCnt },
                { label: "Sold", value: soldCnt },
              ].map(({ label, value }) => (
                <div key={label} className="relative">
                  <span className="text-3xl font-display font-bold text-white block">
                    {value}
                  </span>
                  <span className="text-sm font-bold uppercase tracking-widest text-brand-500">
                    {label}
                  </span>
                </div>
              ))}
            </div>
          </div>
        </div>
      </div>

      {/* Controls */}
      <SearchFilter
        filters={filters}
        onFilterChange={(newFilters) => {
          setFilters((prev) => ({ ...prev, ...newFilters }));
          // Reset to page 1 whenever any filter changes so the
          // user doesn't land on an empty page.
          setPage(1);
        }}
        showFilters={showFilters}
        setShowFilters={setShowFilters}
        totalResults={filtered.length}
      />

      {/* Content */}
      <div className="mx-auto max-w-7xl px-4 sm:px-6 py-12">
        {/* Results count */}
        {!isLoading && !error && (
          <p className="mb-6 text-sm text-gray-500">
            Showing{" "}
            <span className="font-semibold text-gray-700">
              {Math.min((page - 1) * PAGE_SIZE + 1, filtered.length)}
              {" - "}
              {Math.min(page * PAGE_SIZE, filtered.length)}
            </span>{" "}
            of{" "}
            <span className="font-semibold text-gray-700">
              {filtered.length}
            </span>{" "}
            {filtered.length === 1 ? "artwork" : "artworks"}
            {filters.search && (
              <span>
                {" "}
                matching &ldquo;
                <span className="font-medium text-brand-600">{filters.search}</span>
                &rdquo;
              </span>
            )}
          </p>
        )}

        {/* Error state */}
        {error && <ErrorState title="Failed to load listings" message={error} onRetry={load} />}

        {/* Loading state */}
        {isLoading && !error && (
          <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
            {Array.from({ length: PAGE_SIZE }).map((_, i) => (
              <ListingCardSkeleton key={i} />
            ))}
          </div>
        )}

        {/* Empty / No Results state */}
        {!isLoading && !error && filtered.length === 0 && (
          hasActiveFilters ? (
            <NoResults
              message="Try adjusting your search or filters to find what you are looking for."
              onClearFilters={() => {
                setFilters({
                  search: "",
                  status: "All",
                  category: "All",
                  minPrice: "",
                  maxPrice: "",
                  sort: "newest",
                });
              }}
            />
          ) : (
            <EmptyState
              title="No artworks found"
              description="No listings match the current filters. Check back soon for new artworks."
            />
          )
        )}

        {/* Listings grid */}
        {!isLoading && !error && filtered.length > 0 && (
          <>
            <div className="grid gap-6 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
              {paginatedListings.map((listing: Listing) => (
                <ListingCard
                  key={listing.listing_id}
                  listing={listing}
                  onPurchased={load}
                />
              ))}
            </div>

            {/* Pagination */}
            {totalPages > 1 && (
              <div className="mt-10 flex items-center justify-center gap-2">
                <button
                  onClick={() => goToPage(page - 1)}
                  disabled={page <= 1}
                  className="flex items-center gap-1 rounded-xl border border-gray-200 px-3 py-2 text-sm text-gray-600 hover:bg-gray-50 disabled:opacity-40 disabled:cursor-not-allowed transition-all"
                >
                  <ChevronLeft size={16} />
                  Prev
                </button>

                {Array.from({ length: totalPages }, (_, i) => i + 1)
                  .filter((p) => {
                    // Show first, last, and pages near current
                    if (p === 1 || p === totalPages) return true;
                    if (Math.abs(p - page) <= 1) return true;
                    return false;
                  })
                  .reduce<(number | "...")[]>((acc, p, idx, arr) => {
                    if (idx > 0 && p - (arr[idx - 1] as number) > 1) {
                      acc.push("...");
                    }
                    acc.push(p);
                    return acc;
                  }, [])
                  .map((item, idx) =>
                    item === "..." ? (
                      <span
                        key={`dots-${idx}`}
                        className="px-1 text-gray-400"
                      >
                        ...
                      </span>
                    ) : (
                      <button
                        key={item}
                        onClick={() => goToPage(item as number)}
                        className={`min-w-[36px] rounded-xl px-3 py-2 text-sm font-medium transition-all ${
                          page === item
                            ? "bg-brand-500 text-white shadow-md shadow-brand-500/20"
                            : "border border-gray-200 text-gray-600 hover:bg-gray-50"
                        }`}
                      >
                        {item}
                      </button>
                    )
                  )}

                <button
                  onClick={() => goToPage(page + 1)}
                  disabled={page >= totalPages}
                  className="flex items-center gap-1 rounded-xl border border-gray-200 px-3 py-2 text-sm text-gray-600 hover:bg-gray-50 disabled:opacity-40 disabled:cursor-not-allowed transition-all"
                >
                  Next
                  <ChevronRight size={16} />
                </button>
              </div>
            )}
          </>
        )}
      </div>
    </div>
  );
}
