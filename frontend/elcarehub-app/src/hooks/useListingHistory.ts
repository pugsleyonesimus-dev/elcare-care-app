"use client";

/**
 * useListingHistory — paginated provenance history for a listing.
 *
 * Fetches from GET /listings/:id/history with offset/limit pagination.
 * Supports "load more" by appending successive pages to the in-memory list.
 *
 * Supported event types: LISTED, OFFER_SUBMITTED, OFFER_ACCEPTED,
 * PURCHASE, SALE, ROYALTY, CANCELLED, TRANSFER.
 */

import { useState, useEffect, useCallback, useRef } from "react";
import { ActivityEvent, getListingHistory } from "@/lib/indexer";

const PAGE_SIZE = 20;

export interface UseListingHistoryResult {
  events: ActivityEvent[];
  isLoading: boolean;
  isLoadingMore: boolean;
  error: string | null;
  hasMore: boolean;
  loadMore: () => void;
  refresh: () => void;
}

export function useListingHistory(
  listingId: number | null
): UseListingHistoryResult {
  const [events, setEvents] = useState<ActivityEvent[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [isLoadingMore, setIsLoadingMore] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [hasMore, setHasMore] = useState(false);
  const offsetRef = useRef(0);

  const fetchPage = useCallback(
    async (offset: number, append: boolean) => {
      if (listingId === null) return;
      if (append) setIsLoadingMore(true);
      else setIsLoading(true);
      setError(null);
      try {
        const page = await getListingHistory(listingId, offset, PAGE_SIZE);
        setEvents((prev) => (append ? [...prev, ...page.events] : page.events));
        setHasMore(page.hasMore);
        offsetRef.current = offset + page.events.length;
      } catch (err: unknown) {
        setError(
          err instanceof Error ? err.message : "Failed to load history"
        );
      } finally {
        if (append) setIsLoadingMore(false);
        else setIsLoading(false);
      }
    },
    [listingId]
  );

  const refresh = useCallback(() => {
    offsetRef.current = 0;
    fetchPage(0, false);
  }, [fetchPage]);

  const loadMore = useCallback(() => {
    if (!hasMore || isLoadingMore) return;
    fetchPage(offsetRef.current, true);
  }, [hasMore, isLoadingMore, fetchPage]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  return { events, isLoading, isLoadingMore, error, hasMore, loadMore, refresh };
}
