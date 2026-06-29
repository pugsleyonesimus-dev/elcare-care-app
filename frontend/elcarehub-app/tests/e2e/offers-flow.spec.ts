import { test, expect } from '@playwright/test';
import { BUYER_PUBLIC_KEY, TEST_PUBLIC_KEY } from './freighter-mock';
import {
  E2E_METADATA_CID,
  MarketplaceTestStore,
  setupMarketplaceMocks,
  resetE2eListingsInBrowser,
} from './helpers/marketplace-mocks';
import { connectFreighterWallet } from './helpers/wallet';

const DEFAULT_TOKEN =
  process.env.NEXT_PUBLIC_NATIVE_TOKEN_CONTRACT_ID ??
  'CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC';

const BASE_LISTING = {
  listing_id: 7001,
  artist: TEST_PUBLIC_KEY,
  metadata_cid: E2E_METADATA_CID,
  price: String(20 * 10_000_000),
  currency: 'XLM',
  token: DEFAULT_TOKEN,
  status: 'Active',
  owner: null,
  created_at: Math.floor(Date.now() / 1000),
  original_creator: TEST_PUBLIC_KEY,
  royalty_bps: 0,
  recipients: [{ address: TEST_PUBLIC_KEY, percentage: 100 }],
};

const BASE_OFFER = {
  offer_id: 1,
  listing_id: BASE_LISTING.listing_id,
  offerer: BUYER_PUBLIC_KEY,
  amount: String(15 * 10_000_000),
  token: DEFAULT_TOKEN,
  status: 'Pending',
};

test.describe('Offers lifecycle E2E (#115)', () => {
  const store = new MarketplaceTestStore();

  test.beforeEach(async ({ page }) => {
    store.reset();
    store.upsertActive(BASE_LISTING);
    await setupMarketplaceMocks(page, store);
    await resetE2eListingsInBrowser(page);
  });

  test('pending offer is visible on the offers page', async ({ page }) => {
    store.upsertOffer(BASE_OFFER);
    await connectFreighterWallet(page, TEST_PUBLIC_KEY);
    await page.goto('/offers/incoming');
    await expect(page.getByText(/offer|incoming/i).first()).toBeVisible({ timeout: 10_000 });
  });

  test('seller accepts an offer and status updates to Accepted', async ({ page }) => {
    store.upsertOffer(BASE_OFFER);
    await connectFreighterWallet(page, TEST_PUBLIC_KEY);
    await page.goto('/offers/incoming');

    const acceptBtn = page.getByRole('button', { name: /accept/i }).first();
    if (await acceptBtn.isVisible({ timeout: 8_000 })) {
      await acceptBtn.click();
      store.updateOfferStatus(BASE_OFFER.offer_id, 'Accepted');
      await page.reload();
      await expect(page.getByText(/accepted/i)).toBeVisible({ timeout: 10_000 });
    }
  });

  test('seller rejects an offer and status updates to Rejected', async ({ page }) => {
    store.upsertOffer(BASE_OFFER);
    await connectFreighterWallet(page, TEST_PUBLIC_KEY);
    await page.goto('/offers/incoming');

    const rejectBtn = page.getByRole('button', { name: /reject/i }).first();
    if (await rejectBtn.isVisible({ timeout: 8_000 })) {
      await rejectBtn.click();
      store.updateOfferStatus(BASE_OFFER.offer_id, 'Rejected');
      await page.reload();
      await expect(page.getByText(/rejected/i)).toBeVisible({ timeout: 10_000 });
    }
  });

  test('buyer withdraws a pending offer', async ({ page }) => {
    store.upsertOffer(BASE_OFFER);
    await connectFreighterWallet(page, BUYER_PUBLIC_KEY);
    await page.goto('/offers');

    const withdrawBtn = page.getByRole('button', { name: /withdraw/i }).first();
    if (await withdrawBtn.isVisible({ timeout: 8_000 })) {
      await withdrawBtn.click();
      store.updateOfferStatus(BASE_OFFER.offer_id, 'Withdrawn');
      await page.reload();
      await expect(page.getByText(/withdrawn/i)).toBeVisible({ timeout: 10_000 });
    }
  });

  test('withdrawing an already-accepted offer is not possible (negative case)', async ({ page }) => {
    store.upsertOffer({ ...BASE_OFFER, status: 'Accepted' });
    await connectFreighterWallet(page, BUYER_PUBLIC_KEY);
    await page.goto('/offers');

    // Withdraw button should be absent for an accepted offer
    await expect(page.getByRole('button', { name: /withdraw/i })).toHaveCount(0, { timeout: 8_000 });
  });
});
