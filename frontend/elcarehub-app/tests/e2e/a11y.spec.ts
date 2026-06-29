import { test, expect } from '@playwright/test';
import AxeBuilder from '@axe-core/playwright';
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

async function expectNoCriticalViolations(page: import('@playwright/test').Page, context: string) {
  const results = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa'])
    .analyze();

  const critical = results.violations.filter(
    (v) => v.impact === 'critical' || v.impact === 'serious'
  );
  expect(critical, `${context} a11y violations`).toEqual([]);
}

test.describe('Accessibility (page-level)', () => {
  const store = new MarketplaceTestStore();

  test.beforeEach(async ({ page }) => {
    store.reset();
    await setupMarketplaceMocks(page, store);
    await resetE2eListingsInBrowser(page);
  });

  test('home page has no serious/critical violations', async ({ page }) => {
    await page.goto('/');
    await expectNoCriticalViolations(page, 'home');
  });

  test('explore page has no serious/critical violations', async ({ page }) => {
    store.upsertActive({
      listing_id: 9201,
      artist: TEST_PUBLIC_KEY,
      metadata_cid: E2E_METADATA_CID,
      price: String(10 * 10_000_000),
      currency: 'XLM',
      token: DEFAULT_TOKEN,
      status: 'Active',
      owner: null,
      created_at: Math.floor(Date.now() / 1000),
      original_creator: TEST_PUBLIC_KEY,
      royalty_bps: 0,
      recipients: [{ address: TEST_PUBLIC_KEY, percentage: 100 }],
    });

    await page.goto('/explore');
    await expect(page.getByTestId('explore-page')).toBeVisible();
    await expectNoCriticalViolations(page, 'explore');
  });

  test('checkout modal has no serious/critical violations', async ({ page }) => {
    store.upsertActive({
      listing_id: 9202,
      artist: TEST_PUBLIC_KEY,
      metadata_cid: E2E_METADATA_CID,
      price: String(10 * 10_000_000),
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
    await page.goto('/explore');
    await page.getByTestId('buy-now-button').first().click();
    await expect(page.getByTestId('checkout-modal')).toBeVisible();
    await expectNoCriticalViolations(page, 'checkout modal');
  });
});
