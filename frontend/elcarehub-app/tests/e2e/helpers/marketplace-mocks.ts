import { Page } from '@playwright/test';

export const E2E_METADATA_CID = 'QmE2eTestMetadataCid';
export const E2E_IMAGE_CID = 'QmE2eTestImageCid';

export const MOCK_ARTWORK_METADATA = {
  title: 'E2E Serengeti Sunset',
  description: 'Automated test listing',
  artist: 'E2E Artist',
  image: `ipfs://${E2E_IMAGE_CID}`,
  year: '2024',
  category: 'Digital Art',
};

export interface E2eIndexerListing {
  listing_id: number;
  artist: string;
  metadata_cid: string;
  price: string;
  currency: string;
  token: string;
  status: string;
  owner: string | null;
  created_at: number;
  original_creator: string;
  royalty_bps: number;
  recipients: Array<{ address: string; percentage: number }>;
}

export interface E2eIndexerAuction {
  auction_id: number;
  creator: string;
  token: string;
  reserve_price: string;
  highest_bid: string;
  highest_bidder: string | null;
  end_time: number;
  status: string; // 'Active' | 'Finalized' | 'Cancelled'
}

export interface E2eIndexerOffer {
  offer_id: number;
  listing_id: number;
  offerer: string;
  amount: string;
  token: string;
  status: string; // 'Pending' | 'Accepted' | 'Rejected' | 'Withdrawn'
}

/** Shared across pages in a test — indexer mock reads from here. */
export class MarketplaceTestStore {
  listings: E2eIndexerListing[] = [];
  auctions: E2eIndexerAuction[] = [];
  offers: E2eIndexerOffer[] = [];

  reset() {
    this.listings = [];
    this.auctions = [];
    this.offers = [];
  }

  upsertActive(listing: E2eIndexerListing) {
    this.listings = this.listings.filter((l) => l.listing_id !== listing.listing_id);
    this.listings.push(listing);
  }

  markSold(listingId: number, buyer: string) {
    this.listings = this.listings.map((l) =>
      l.listing_id === listingId ? { ...l, status: 'Sold', owner: buyer } : l
    );
  }

  activeListings() {
    return this.listings.filter((l) => l.status === 'Active');
  }

  upsertAuction(auction: E2eIndexerAuction) {
    this.auctions = this.auctions.filter((a) => a.auction_id !== auction.auction_id);
    this.auctions.push(auction);
  }

  placeBid(auctionId: number, bidder: string, amount: string) {
    this.auctions = this.auctions.map((a) =>
      a.auction_id === auctionId
        ? { ...a, highest_bid: amount, highest_bidder: bidder }
        : a
    );
  }

  finalizeAuction(auctionId: number) {
    this.auctions = this.auctions.map((a) =>
      a.auction_id === auctionId ? { ...a, status: 'Finalized' } : a
    );
  }

  upsertOffer(offer: E2eIndexerOffer) {
    this.offers = this.offers.filter((o) => o.offer_id !== offer.offer_id);
    this.offers.push(offer);
  }

  updateOfferStatus(offerId: number, status: string) {
    this.offers = this.offers.map((o) =>
      o.offer_id === offerId ? { ...o, status } : o
    );
  }
}

const INDEXER_URL = (
  process.env.NEXT_PUBLIC_INDEXER_URL ?? 'http://localhost:4000'
).replace(/\/$/, '');

/**
 * Mocks IPFS uploads, metadata gateway reads, and indexer listing/auction/offer APIs.
 * Chain calls use NEXT_PUBLIC_E2E_MOCK_CHAIN on the dev server.
 */
export async function setupMarketplaceMocks(page: Page, store: MarketplaceTestStore) {
  await page.route('**/api/ipfs/upload-image', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ cid: E2E_IMAGE_CID }),
    });
  });

  await page.route('**/api/ipfs/upload-metadata', async (route) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ cid: E2E_METADATA_CID }),
    });
  });

  const fulfillMetadata = async (route: { fulfill: (opts: object) => Promise<void> }) => {
    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(MOCK_ARTWORK_METADATA),
    });
  };

  await page.route('**/gateway.pinata.cloud/ipfs/**', fulfillMetadata);
  await page.route('**/ipfs.io/ipfs/**', fulfillMetadata);

  await page.route(`${INDEXER_URL}/listings**`, async (route) => {
    if (route.request().method() !== 'GET') {
      return route.continue();
    }

    const url = new URL(route.request().url());
    const statusFilter = url.searchParams.get('status');
    let rows = store.listings;
    if (statusFilter && statusFilter !== 'All') {
      rows = rows.filter((l) => l.status === statusFilter);
    }

    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ listings: rows, total: rows.length }),
    });
  });

  await page.route(`${INDEXER_URL}/auctions**`, async (route) => {
    if (route.request().method() !== 'GET') return route.continue();

    const url = new URL(route.request().url());
    const idMatch = url.pathname.match(/\/auctions\/(\d+)/);
    if (idMatch) {
      const auction = store.auctions.find((a) => a.auction_id === Number(idMatch[1]));
      if (!auction) {
        await route.fulfill({ status: 404, body: JSON.stringify({ error: 'Not found' }) });
      } else {
        await route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify(auction) });
      }
      return;
    }

    const statusFilter = url.searchParams.get('status');
    let rows = store.auctions;
    if (statusFilter) rows = rows.filter((a) => a.status === statusFilter);

    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(rows),
    });
  });

  await page.route(`${INDEXER_URL}/offers**`, async (route) => {
    if (route.request().method() !== 'GET') return route.continue();

    const url = new URL(route.request().url());
    const listingId = url.searchParams.get('listing_id');
    let rows = store.offers;
    if (listingId) rows = rows.filter((o) => o.listing_id === Number(listingId));

    await route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify(rows),
    });
  });
}

export async function resetE2eListingsInBrowser(page: Page) {
  await page.evaluate(() => {
    (window as Window & { __E2E_RESET_LISTINGS__?: () => void }).__E2E_RESET_LISTINGS__?.();
  });
}
