// In-memory chain mock for Playwright E2E (NEXT_PUBLIC_E2E_MOCK_CHAIN=true).

import { DEFAULT_TOKEN } from "@/config/tokens";
import type { Listing } from "./contract";

let nextListingId = 9001;
const listings = new Map<number, Listing>();

declare global {
  interface Window {
    __E2E_GET_LISTINGS__?: () => Listing[];
    __E2E_RESET_LISTINGS__?: () => void;
    __E2E_UPSERT_LISTING__?: (listing: Listing) => void;
  }
}

export function isE2eMockChain(): boolean {
  return process.env.NEXT_PUBLIC_E2E_MOCK_CHAIN === "true";
}

export function resetE2eMockListings(): void {
  listings.clear();
  nextListingId = 9001;
}

export function getE2eMockListings(): Listing[] {
  return Array.from(listings.values());
}

export function registerE2eMockListingsOnWindow(): void {
  if (typeof window === "undefined") return;
  window.__E2E_GET_LISTINGS__ = getE2eMockListings;
  window.__E2E_RESET_LISTINGS__ = resetE2eMockListings;
  window.__E2E_UPSERT_LISTING__ = e2eMockUpsertListing;
}

export function e2eMockUpsertListing(listing: Listing): void {
  listings.set(listing.listing_id, listing);
  if (listing.listing_id >= nextListingId) {
    nextListingId = listing.listing_id + 1;
  }
}

export function e2eMockCreateListing(
  artistPublicKey: string,
  price: number,
  tokenAddress: string = DEFAULT_TOKEN.address,
  collectionAddress: string,
  nftTokenId: number
): number {
  const id = nextListingId++;
  const priceStroops = BigInt(Math.round(price * 10_000_000));
  listings.set(id, {
    listing_id: id,
    artist: artistPublicKey,
    collection: collectionAddress,
    token_id: nftTokenId,
    price: priceStroops,
    currency: DEFAULT_TOKEN.symbol,
    token: tokenAddress,
    recipients: [{ address: artistPublicKey, percentage: 100 }],
    status: "Active",
    owner: null,
    created_at: Math.floor(Date.now() / 1000),
  });
  return id;
}

export function e2eMockBuyArtwork(buyerPublicKey: string, listingId: number): boolean {
  const listing = listings.get(listingId);
  if (!listing || listing.status !== "Active") {
    throw new Error("Listing is not available for purchase.");
  }
  if (listing.artist === buyerPublicKey) {
    throw new Error("Cannot buy your own listing.");
  }
  listing.status = "Sold";
  listing.owner = buyerPublicKey;
  return true;
}
