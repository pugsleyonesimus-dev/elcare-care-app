// ─────────────────────────────────────────────────────────────
// lib/indexer.ts — ELCARE-HUB HTTP indexer client
// ─────────────────────────────────────────────────────────────

import axios, { AxiosError, isAxiosError } from "axios";
import { config } from "./config";

const DEFAULT_TIMEOUT_MS = 12_000;
const MAX_RETRIES = 3;
const RETRY_DELAY_MS = 500;

export interface ActivityEvent {
  id: string;
  type: "PURCHASE" | "LISTED" | "CANCELLED" | "SALE" | "ROYALTY";
  listing_id: number;
  title: string;
  price: string;
  timestamp: number;
  from: string;
  to: string;
  tx_hash: string;
}

interface RoyaltyStatsResponse {
  totalEarned: string;
  payoutCount: number;
  lastPayout: number;
}

export interface IndexerCollectionRow {
  id: number;
  contractAddress: string;
  kind: string;
  creator: string;
  name: string | null;
  symbol: string | null;
  deployedAtLedger: number;
  createdAt?: string;
}

export interface CollectionFilter {
  kind?: string;
  creator?: string;
  page?: number;
  limit?: number;
}

/** Raw event row as returned by the indexer API and stored in Prisma. */
interface RawMarketplaceEvent {
  id: number;
  listingId?: string | null;
  eventType: string;
  actor: string;
  data: Record<string, unknown>;
  ledgerSequence: number;
  ledgerTimestamp?: string;
}

function sleep(ms: number) {
  return new Promise((r) => setTimeout(r, ms));
}

function isTransientAxiosError(e: unknown): boolean {
  if (!isAxiosError(e)) return false;
  if (e.code === "ECONNABORTED" || e.code === "ETIMEDOUT") return true;
  const s = e.response?.status;
  return s === undefined || s >= 500;
}

async function httpGet<T>(url: string): Promise<T> {
  const res = await axios.get<T>(url, {
    timeout: DEFAULT_TIMEOUT_MS,
    validateStatus: (s) => s < 400,
  });
  return res.data;
}

async function fetchWithRetry<T>(path: string): Promise<T> {
  const url = `${config.indexerUrl}${path}`;
  let lastErr: unknown;
  for (let attempt = 0; attempt < MAX_RETRIES; attempt++) {
    try {
      return await httpGet<T>(url);
    } catch (e) {
      lastErr = e;
      const retry =
        attempt < MAX_RETRIES - 1 && isTransientAxiosError(e as AxiosError);
      if (!retry) {
        throw e instanceof Error ? e : new Error(String(e));
      }
      await sleep(RETRY_DELAY_MS * (attempt + 1));
    }
  }
  throw lastErr instanceof Error
    ? lastErr
    : new Error("Indexer request failed");
}

function isNonEmptyString(v: unknown): v is string {
  return typeof v === "string" && v.length > 0;
}

function isRawMarketplaceEvent(v: unknown): v is RawMarketplaceEvent {
  if (v === null || typeof v !== "object") return false;
  const o = v as Record<string, unknown>;
  return (
    typeof o.eventType === "string" &&
    typeof o.actor === "string" &&
    typeof o.ledgerSequence === "number" &&
    typeof o.data === "object" &&
    o.data !== null
  );
}

function parseActivityList(data: unknown): RawMarketplaceEvent[] {
  if (!Array.isArray(data)) return [];
  return data.filter(isRawMarketplaceEvent);
}

function addrString(x: unknown): string {
  if (typeof x === "string") return x;
  return "";
}

function isRoyaltyStatsResponse(v: unknown): v is RoyaltyStatsResponse {
  if (v === null || typeof v !== "object") return false;
  const o = v as Record<string, unknown>;
  return (
    typeof o.totalEarned === "string" &&
    typeof o.payoutCount === "number" &&
    Number.isFinite(o.payoutCount) &&
    typeof o.lastPayout === "number" &&
    Number.isFinite(o.lastPayout)
  );
}

function isIndexerCollectionRow(v: unknown): v is IndexerCollectionRow {
  if (v === null || typeof v !== "object") return false;
  const o = v as Record<string, unknown>;
  return (
    typeof o.contractAddress === "string" &&
    typeof o.kind === "string" &&
    typeof o.creator === "string"
  );
}

function eventTypeToActivity(
  eventType: string
): ActivityEvent["type"] {
  switch (eventType) {
    case "LISTING_CREATED":
      return "LISTED";
    case "ARTWORK_SOLD":
      return "PURCHASE";
    case "LISTING_CANCELLED":
      return "CANCELLED";
    default:
      return "SALE";
  }
}

/**
 * Fetches marketplace-related events for a wallet from the ELCARE-HUB indexer.
 */
export async function getWalletActivity(
  publicKey: string
): Promise<ActivityEvent[]> {
  if (!isNonEmptyString(publicKey)) return [];
  try {
    const raw = await fetchWithRetry<unknown>(
      `/wallets/${encodeURIComponent(publicKey)}/activity?limit=50`
    );
    return parseActivityList(raw).map((ev) =>
      mapWalletEventToActivity(ev, publicKey)
    );
  } catch (e) {
    console.warn(
      "[indexer] getWalletActivity:",
      e instanceof Error ? e.message : e
    );
    return [];
  }
}

function mapWalletEventToActivity(
  ev: RawMarketplaceEvent,
  publicKey: string
): ActivityEvent {
  const data = ev.data;
  const listingIdRaw = ev.listingId;
  const listingIdNum = listingIdRaw != null ? Number(listingIdRaw) : 0;
  const ts = ev.ledgerTimestamp
    ? new Date(ev.ledgerTimestamp).getTime()
    : Date.now();
  const buyer = addrString(data.buyer);
  const artist = addrString(data.artist);
  const price = data.price != null ? String(data.price) : "0";

  let type = eventTypeToActivity(ev.eventType);
  let from = ev.actor;
  let to = ev.actor;

  if (ev.eventType === "LISTING_CREATED") {
    from = artist || ev.actor;
    to = config.contractId || "Contract";
  } else if (ev.eventType === "ARTWORK_SOLD") {
    if (buyer === publicKey) {
      type = "PURCHASE";
      from = artist;
      to = publicKey;
    } else {
      type = "SALE";
      from = artist;
      to = buyer;
    }
  } else if (ev.eventType === "LISTING_CANCELLED") {
    type = "CANCELLED";
    to = "—";
  }

  return {
    id: `ix_${ev.id}`,
    type,
    listing_id: Number.isFinite(listingIdNum) ? listingIdNum : 0,
    title: `Listing #${listingIdNum || "—"}`,
    price,
    timestamp: ts,
    from: from || "—",
    to: to || "—",
    tx_hash: `ledger_${ev.ledgerSequence}`,
  };
}

/**
 * Estimates total royalties for an artist from indexed sales (listing rows).
 */
export async function getRoyaltyStats(
  publicKey: string
): Promise<RoyaltyStatsResponse> {
  const empty: RoyaltyStatsResponse = {
    totalEarned: "0",
    payoutCount: 0,
    lastPayout: 0,
  };
  if (!isNonEmptyString(publicKey)) return empty;
  try {
    const data = await fetchWithRetry<unknown>(
      `/wallets/${encodeURIComponent(publicKey)}/royalty-stats`
    );
    if (isRoyaltyStatsResponse(data)) return data;
  } catch (e) {
    console.warn(
      "[indexer] getRoyaltyStats:",
      e instanceof Error ? e.message : e
    );
  }
  return empty;
}

/**
 * Fetches activity (event timeline) for a specific marketplace listing.
 */
export async function getListingActivity(
  listingId: number
): Promise<ActivityEvent[]> {
  if (!Number.isFinite(listingId)) return [];
  try {
    const raw = await fetchWithRetry<unknown>(
      `/listings/${listingId}/history`
    );
    return parseActivityList(raw).map((ev) =>
      mapListingHistoryEvent(ev, listingId)
    );
  } catch (e) {
    console.warn(
      "[indexer] getListingActivity:",
      e instanceof Error ? e.message : e
    );
    return [];
  }
}

function mapListingHistoryEvent(
  ev: RawMarketplaceEvent,
  listingId: number
): ActivityEvent {
  const data = ev.data;
  const ts = ev.ledgerTimestamp
    ? new Date(ev.ledgerTimestamp).getTime()
    : Date.now();
  const priceField =
    data.price ?? (data as { new_price?: unknown }).new_price;
  const price = priceField != null ? String(priceField) : "0";
  const artist = addrString(data.artist);
  const buyer = addrString(data.buyer);

  return {
    id: `lst_${ev.id}`,
    type: eventTypeToActivity(ev.eventType),
    listing_id: listingId,
    title: "Artwork",
    price,
    timestamp: ts,
    from: artist || ev.actor,
    to: buyer || config.contractId,
    tx_hash: `ledger_${ev.ledgerSequence}`,
  };
}

/**
 * Deployed collections from the indexer (Supplementary to on-chain `all_collections` when the indexer is synced).
 */
export async function getCollections(
  filter: CollectionFilter = {}
): Promise<{ collections: IndexerCollectionRow[]; total: number }> {
  const params = new URLSearchParams();
  if (filter.kind) params.set("kind", filter.kind);
  if (filter.creator) params.set("creator", filter.creator);
  if (filter.limit != null) params.set("limit", String(filter.limit));
  if (filter.page != null) params.set("page", String(filter.page));
  const q = params.toString();
  try {
    const raw = await fetchWithRetry<unknown>(
      `/collections${q ? `?${q}` : ""}`
    );
    if (!Array.isArray(raw)) return { collections: [], total: 0 };
    const collections = raw.filter(isIndexerCollectionRow);
    return { collections, total: collections.length };
  } catch (e) {
    console.warn(
      "[indexer] getCollections:",
      e instanceof Error ? e.message : e
    );
    return { collections: [], total: 0 };
  }
}

/**
 * Fetch royalty stats for an artist (alias for getRoyaltyStats for server-side usage)
 */
export async function fetchRoyaltyStats(
  publicKey: string
): Promise<RoyaltyStatsResponse> {
  return getRoyaltyStats(publicKey);
}

/**
 * Fetch artist listings from the indexer
 */
export async function fetchArtistListings(
  publicKey: string
): Promise<any[]> {
  if (!isNonEmptyString(publicKey)) return [];
  try {
    const data = await fetchWithRetry<unknown>(
      `/listings?artist=${encodeURIComponent(publicKey)}`
    );
    if (Array.isArray(data)) return data;
    return [];
  } catch (e) {
    console.warn(
      "[indexer] fetchArtistListings:",
      e instanceof Error ? e.message : e
    );
    return [];
  }
}

/**
 * Fetch listings from the indexer with optional filters and pagination.
 * Throws if the indexer is unreachable so callers can fall back to on-chain.
 */
export async function fetchListings(options: {
  status?: string;
  limit?: number;
  offset?: number;
  minPrice?: string;
  maxPrice?: string;
  search?: string;
} = {}): Promise<{ listings: any[]; total?: number }> {
  const params = new URLSearchParams();
  if (options.status) params.set('status', options.status);
  if (options.limit != null) params.set('limit', String(options.limit));
  if (options.offset != null) params.set('offset', String(options.offset));
  if (options.minPrice) params.set('minPrice', options.minPrice);
  if (options.maxPrice) params.set('maxPrice', options.maxPrice);
  if (options.search) params.set('search', options.search);
  const q = params.toString();

  const raw = await fetchWithRetry<unknown>(`/listings${q ? `?${q}` : ''}`);
  if (raw == null) return { listings: [] };
  
  // If paginated, indexer returns { listings, total }
  if (typeof raw === 'object' && (raw as any).listings) {
    const r = raw as any;
    return { listings: Array.isArray(r.listings) ? r.listings : [], total: r.total };
  }
  if (Array.isArray(raw)) return { listings: raw };
  return { listings: [] };
}

/**
 * Fetch auctions from the indexer with optional filters.
 * Throws if the indexer is unreachable so callers can fall back to on-chain.
 */
export async function fetchAuctions(options: {
  creator?: string;
  status?: string;
} = {}): Promise<any[]> {
  const params = new URLSearchParams();
  if (options.creator) params.set('creator', options.creator);
  if (options.status) params.set('status', options.status);
  const q = params.toString();
  const raw = await fetchWithRetry<unknown>(`/auctions${q ? `?${q}` : ''}`);
  if (Array.isArray(raw)) return raw;
  return [];
}

/**
 * Fetch a single listing (with optional metadata) from the indexer.
 */
export async function fetchListingById(id: number): Promise<any | null> {
  if (!Number.isFinite(id)) return null;
  try {
    const raw = await fetchWithRetry<unknown>(`/listings/${id}`);
    return raw as any;
  } catch (e) {
    console.warn('[indexer] fetchListingById:', e instanceof Error ? e.message : e);
    return null;
  }
}

// ─────────────────────────────────────────────────────────────
// SSE client — live marketplace event streaming (ISSUE-061)
// ─────────────────────────────────────────────────────────────

/** Relevant SSE event types emitted by the indexer stream. */
export type MarketplaceSSEEventType =
  | "LISTING_CREATED"
  | "LISTING_CANCELLED"
  | "ARTWORK_SOLD"
  | "BID_PLACED"
  | "AUCTION_FINALIZED"
  | "AUCTION_CANCELLED"
  | "AUCTION_EXTENDED";

export interface MarketplaceSSEEvent {
  type: MarketplaceSSEEventType;
  listingId?: number;
  auctionId?: number;
  data?: Record<string, unknown>;
}

/** Options for {@link subscribeToMarketplaceEvents}. */
export interface SSESubscribeOptions {
  /** Called for each parsed marketplace event. */
  onEvent: (event: MarketplaceSSEEvent) => void;
  /** Called when the connection opens (or re-opens after a reconnect). */
  onOpen?: () => void;
  /** Called when the connection closes permanently (max retries exhausted). */
  onClose?: () => void;
  /**
   * The last event ID received by a previous session.
   * Passed as `Last-Event-ID` so the server can replay missed events.
   */
  lastEventId?: string;
  /**
   * Maximum number of reconnect attempts before giving up.
   * Defaults to 10.
   */
  maxRetries?: number;
  /**
   * Base delay in milliseconds between reconnect attempts.
   * Uses exponential back-off capped at 30 s.
   * Defaults to 1 000 ms.
   */
  baseRetryDelayMs?: number;
  /**
   * Debounce window in milliseconds.  When multiple events arrive rapidly
   * within this window they are batched and the callback is invoked once
   * per unique event at the end of the window.
   * Defaults to 200 ms.  Set to 0 to disable debouncing.
   */
  debounceMs?: number;
}

/** Handle returned by {@link subscribeToMarketplaceEvents}.  Call `close()` to unsubscribe. */
export interface SSESubscription {
  /** Unsubscribes and releases all resources. */
  close: () => void;
  /** The last `id` field seen on any SSE event, for use as `lastEventId` on reconnect. */
  getLastEventId: () => string | null;
}

const SSE_RELEVANT_TYPES = new Set<string>([
  "LISTING_CREATED",
  "LISTING_CANCELLED",
  "ARTWORK_SOLD",
  "BID_PLACED",
  "AUCTION_FINALIZED",
  "AUCTION_CANCELLED",
  "AUCTION_EXTENDED",
]);

function parseSSEData(rawData: string): MarketplaceSSEEvent | null {
  try {
    const parsed = JSON.parse(rawData) as Record<string, unknown>;
    const type = (parsed.type ?? parsed.eventType) as string | undefined;
    if (!type || !SSE_RELEVANT_TYPES.has(type)) return null;
    return {
      type: type as MarketplaceSSEEventType,
      listingId:
        parsed.listingId != null ? Number(parsed.listingId) : undefined,
      auctionId:
        parsed.auctionId != null ? Number(parsed.auctionId) : undefined,
      data:
        typeof parsed.data === "object" && parsed.data !== null
          ? (parsed.data as Record<string, unknown>)
          : undefined,
    };
  } catch {
    return null;
  }
}

/**
 * Opens a persistent SSE connection to the indexer's `/events/stream` endpoint.
 *
 * Features:
 * - Exponential back-off reconnect with configurable max retries.
 * - `Last-Event-ID` header forwarded on reconnect so the server can replay
 *   missed events (pairs with ISSUE-061).
 * - Debounce/batching of rapid events within a configurable window.
 * - Returns a {@link SSESubscription} handle — call `.close()` on unmount.
 *
 * The implementation falls back gracefully when `EventSource` is unavailable
 * (e.g., SSR / test environments).
 */
export function subscribeToMarketplaceEvents(
  indexerUrl: string,
  opts: SSESubscribeOptions
): SSESubscription {
  const {
    onEvent,
    onOpen,
    onClose,
    lastEventId: initialLastEventId = null,
    maxRetries = 10,
    baseRetryDelayMs = 1_000,
    debounceMs = 200,
  } = opts;

  let es: EventSource | null = null;
  let retryCount = 0;
  let closed = false;
  let lastEventId: string | null = initialLastEventId ?? null;
  let debounceTimer: ReturnType<typeof setTimeout> | null = null;
  let pendingEvents: MarketplaceSSEEvent[] = [];

  function flushPending() {
    debounceTimer = null;
    const eventsToFlush = pendingEvents.splice(0);
    for (const ev of eventsToFlush) {
      onEvent(ev);
    }
  }

  function dispatchEvent(ev: MarketplaceSSEEvent) {
    if (debounceMs <= 0) {
      onEvent(ev);
      return;
    }
    pendingEvents.push(ev);
    if (debounceTimer !== null) clearTimeout(debounceTimer);
    debounceTimer = setTimeout(flushPending, debounceMs);
  }

  function buildUrl(): string {
    const base = `${indexerUrl}/events/stream`;
    if (lastEventId) {
      return `${base}?lastEventId=${encodeURIComponent(lastEventId)}`;
    }
    return base;
  }

  function connect() {
    if (closed) return;
    if (typeof EventSource === "undefined") return; // SSR or test env without polyfill

    try {
      es = new EventSource(buildUrl());
    } catch {
      scheduleReconnect();
      return;
    }

    es.onopen = () => {
      retryCount = 0;
      onOpen?.();
    };

    es.onmessage = (msgEvent: MessageEvent) => {
      // Track Last-Event-ID for reconnect
      if (msgEvent.lastEventId) {
        lastEventId = msgEvent.lastEventId;
      }
      const parsed = parseSSEData(msgEvent.data as string);
      if (parsed) dispatchEvent(parsed);
    };

    // Also handle named events the server may emit
    for (const evType of SSE_RELEVANT_TYPES) {
      es.addEventListener(evType, (msgEvent: Event) => {
        const msg = msgEvent as MessageEvent;
        if (msg.lastEventId) lastEventId = msg.lastEventId;
        const parsed = parseSSEData(msg.data as string);
        if (parsed) dispatchEvent(parsed);
      });
    }

    es.onerror = () => {
      es?.close();
      es = null;
      scheduleReconnect();
    };
  }

  function scheduleReconnect() {
    if (closed || retryCount >= maxRetries) {
      onClose?.();
      return;
    }
    const delay = Math.min(baseRetryDelayMs * Math.pow(2, retryCount), 30_000);
    retryCount += 1;
    setTimeout(connect, delay);
  }

  connect();

  return {
    close() {
      closed = true;
      if (debounceTimer !== null) {
        clearTimeout(debounceTimer);
        flushPending();
      }
      es?.close();
      es = null;
    },
    getLastEventId() {
      return lastEventId;
    },
  };
}
