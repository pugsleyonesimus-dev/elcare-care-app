// ─────────────────────────────────────────────────────────────
// hooks/useFilterUrlSync.ts — Sync SearchFilter state ↔ URL
// ─────────────────────────────────────────────────────────────
//
// Reads the initial filter state from URL query params on mount
// and exposes a `syncToUrl` function that performs a shallow
// router replace so the URL stays shareable and survives reloads.
//
// ─────────────────────────────────────────────────────────────

"use client";

import { useCallback, useRef } from "react";
import { useRouter, useSearchParams, usePathname } from "next/navigation";
import type { Filters } from "@/components/SearchFilter";

/** Return type for {@link useFilterUrlSync}. */
export interface FilterUrlSync {
  /** Initial filter values parsed from the URL on mount. */
  initialFilters: Filters;
  /** Initial page number parsed from the URL on mount. */
  initialPage: number;
  /**
   * Call this whenever filters or page change to keep the URL
   * in sync.  Uses `router.replace` (shallow) so the page does
   * not scroll or re-fetch server data.
   */
  syncToUrl: (filters: Filters, page: number) => void;
}

/**
 * Reads initial `Filters` from URL search params (`q`, `status`,
 * `category`, `minPrice`, `maxPrice`, `sort`, `page`) and
 * provides a `syncToUrl` function to push filter changes back
 * to the URL with shallow routing.
 *
 * @example
 * ```tsx
 * const { initialFilters, initialPage, syncToUrl } = useFilterUrlSync();
 * const [filters, setFilters] = useState<Filters>(initialFilters);
 *
 * useEffect(() => { syncToUrl(filters, page); }, [filters, page]);
 * ```
 */
export function useFilterUrlSync(): FilterUrlSync {
  const router = useRouter();
  const pathname = usePathname();
  const searchParams = useSearchParams();

  // Snapshot initial values once on mount (via ref) so that a
  // subsequent router.replace from syncToUrl doesn't read stale
  // values on the next render.
  const initialFilters = useRef<Filters>({
    search: searchParams.get("q") ?? "",
    status: (searchParams.get("status") as Filters["status"]) ?? "All",
    category: searchParams.get("category") ?? "All",
    minPrice: searchParams.get("minPrice") ?? "",
    maxPrice: searchParams.get("maxPrice") ?? "",
    sort: (searchParams.get("sort") as Filters["sort"]) ?? "newest",
  }).current;

  const initialPage = useRef(
    Math.max(1, parseInt(searchParams.get("page") ?? "1", 10) || 1),
  ).current;

  const syncToUrl = useCallback(
    (filters: Filters, page: number) => {
      const params = new URLSearchParams();

      if (filters.search) params.set("q", filters.search);
      if (filters.status !== "All") params.set("status", filters.status);
      if (filters.category !== "All")
        params.set("category", filters.category);
      if (filters.minPrice) params.set("minPrice", filters.minPrice);
      if (filters.maxPrice) params.set("maxPrice", filters.maxPrice);
      if (filters.sort !== "newest") params.set("sort", filters.sort);
      if (page > 1) params.set("page", String(page));

      const qs = params.toString();
      const url = qs ? `${pathname}?${qs}` : pathname;

      // Shallow replace — no server re-fetch, no scroll change
      router.replace(url, { scroll: false });
    },
    [router, pathname],
  );

  return { initialFilters, initialPage, syncToUrl };
}
