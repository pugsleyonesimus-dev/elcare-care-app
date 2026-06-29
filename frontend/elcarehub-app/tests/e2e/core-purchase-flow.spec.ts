import { test, expect } from '@playwright/test';
import { BUYER_PUBLIC_KEY, TEST_PUBLIC_KEY } from './freighter-mock';
import {
  E2E_METADATA_CID,
  MarketplaceTestStore,
  MOCK_ARTWORK_METADATA,
  setupMarketplaceMocks,
  resetE2eListingsInBrowser,
  seedE2eChainListing,
} from './helpers/marketplace-mocks';
import { connectFreighterWallet } from './helpers/wallet';

const DEFAULT_TOKEN =
  process.env.NEXT_PUBLIC_NATIVE_TOKEN_CONTRACT_ID ??
  'CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC';

const LISTING_ID = 9101;

test.describe('Core purchase flow (mock chain)', () => {
  const store = new MarketplaceTestStore();

  test.beforeEach(async ({ page }) => {
    store.reset();
    await setupMarketplaceMocks(page, store);
    await resetE2eListingsInBrowser(page);
  });

  test('connect → browse → open listing → checkout → success', async ({ page }) => {
    store.upsertActive({
      listing_id: LISTING_ID,
      artist: TEST_PUBLIC_KEY,
      metadata_cid: E2E_METADATA_CID,
      price: String(15 * 10_000_000),
      currency: 'XLM',
      token: DEFAULT_TOKEN,
      status: 'Active',
      owner: null,
      created_at: Math.floor(Date.now() / 1000),
      original_creator: TEST_PUBLIC_KEY,
      royalty_bps: 0,
      recipients: [{ address: TEST_PUBLIC_KEY, percentage: 100 }],
    });

    await connectFreighterWallet(page, BUYER_PUBLIC_KEY);

    await seedE2eChainListing(page, {
      listing_id: LISTING_ID,
      artist: TEST_PUBLIC_KEY,
      metadata_cid: E2E_METADATA_CID,
      price: String(15 * 10_000_000),
      token: DEFAULT_TOKEN,
    });

    await page.goto('/explore');
    await expect(page.getByTestId('explore-page')).toBeVisible();
    await expect(page.getByText(MOCK_ARTWORK_METADATA.title)).toBeVisible();

    const listingCard = page.getByTestId(`listing-card-${LISTING_ID}`);
    await listingCard.getByTestId('buy-now-button').click();
    await expect(page.getByTestId('checkout-modal')).toBeVisible();

    await page.getByTestId('checkout-pay-button').click();
    await page.getByTestId('checkout-pay-button').click();
    await expect(page.getByTestId('purchase-success')).toBeVisible({ timeout: 15_000 });
    await expect(page.getByTestId('checkout-modal')).toBeHidden();

    store.markSold(LISTING_ID, BUYER_PUBLIC_KEY);
    await page.goto('/explore');
    await expect(page.getByTestId(`listing-card-${LISTING_ID}`)).toContainText('Sold');
  });
});
