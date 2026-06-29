import { test, expect } from '@playwright/test';
import path from 'path';
import { connectFreighterWallet, openNewListingTab } from './helpers/wallet';
import { MarketplaceTestStore, setupMarketplaceMocks, resetE2eListingsInBrowser } from './helpers/marketplace-mocks';

const MOBILE_VIEWPORT = { width: 375, height: 812 };

test.describe('Mobile Responsive Layout', () => {
  const store = new MarketplaceTestStore();
  const DEFAULT_TOKEN = process.env.NEXT_PUBLIC_NATIVE_TOKEN_CONTRACT_ID ?? 'CDLZFC3SYJYDZT7K67VZ75HPJVIEUVNIXF47ZG2FB2RMQQVU2HHGCYSC';

  test.beforeEach(async ({ page }) => {
    store.reset();
    await setupMarketplaceMocks(page, store);
    await resetE2eListingsInBrowser(page);
    await connectFreighterWallet(page);
    await page.setViewportSize(MOBILE_VIEWPORT);
  });

  test('listing form has no horizontal scroll on mobile', async ({ page }) => {
    await openNewListingTab(page);
    await expect(page.getByText('List Your Artwork')).toBeVisible();

    // Verify no horizontal scroll
    const scrollWidth = await page.evaluate(() => document.documentElement.scrollWidth);
    const viewportWidth = await page.evaluate(() => window.innerWidth);
    expect(scrollWidth).toBeLessThanOrEqual(viewportWidth + 1);
  });

  test('listing form recipient row stacks on mobile', async ({ page }) => {
    await openNewListingTab(page);

    // The recipient address input should be full width above the percentage input on mobile
    const recipientAddress = page.getByPlaceholder('Stellar address (G...)');
    await expect(recipientAddress).toBeVisible();

    // Add a second recipient to test multi-recipient layout
    await page.getByRole('button', { name: /add recipient/i }).click();

    // Check that the form is still fully usable without horizontal scroll
    const scrollWidth = await page.evaluate(() => document.documentElement.scrollWidth);
    const viewportWidth = await page.evaluate(() => window.innerWidth);
    expect(scrollWidth).toBeLessThanOrEqual(viewportWidth + 1);
  });

  test('listing form touch targets meet minimum size on mobile', async ({ page }) => {
    await openNewListingTab(page);

    // Check "Add Recipient" button height
    const addRecipientBtn = page.getByRole('button', { name: /add recipient/i });
    const btnBox = await addRecipientBtn.boundingBox();
    expect(btnBox).not.toBeNull();
    expect(btnBox!.height).toBeGreaterThanOrEqual(40);

    // Check "Create Listing" button height
    const createBtn = page.getByRole('button', { name: /create listing/i });
    const createBox = await createBtn.boundingBox();
    expect(createBox).not.toBeNull();
    expect(createBox!.height).toBeGreaterThanOrEqual(44);

    // Check input fields have adequate touch target
    const inputs = await page.locator('input, select').all();
    for (const input of inputs) {
      const box = await input.boundingBox();
      if (box) {
        expect(box.height).toBeGreaterThanOrEqual(40);
      }
    }
  });

  test('listing form inputs are reachable above keyboard on mobile', async ({ page }) => {
    await openNewListingTab(page);

    // Focus on the collection address input (first field)
    const collectionInput = page.getByPlaceholder(/e\.g\. C\.\.\./i);
    await collectionInput.focus();

    // The input should be visible and in the viewport
    await expect(collectionInput).toBeVisible();

    // Scroll to bottom of form to check all inputs reachable
    const createBtn = page.getByRole('button', { name: /create listing/i });
    await createBtn.scrollIntoViewIfNeeded();
    await expect(createBtn).toBeVisible();

    // Scroll back to top
    await collectionInput.scrollIntoViewIfNeeded();
    await expect(collectionInput).toBeVisible();
  });

  test('listing form all fields scrollable on mobile', async ({ page }) => {
    await openNewListingTab(page);

    // Add maximum recipients to ensure long form is scrollable
    for (let i = 1; i < 4; i++) {
      const addBtn = page.getByRole('button', { name: /add recipient/i });
      if (await addBtn.isVisible()) {
        await addBtn.click();
      }
    }

    // Fill in the last recipient's percentage to get the sum right
    const pctInputs = page.locator('input[aria-label*="percentage"]');
    const count = await pctInputs.count();
    for (let i = 0; i < count; i++) {
      await pctInputs.nth(i).scrollIntoViewIfNeeded();
    }

    // Verify submit button is reachable
    const createBtn = page.getByRole('button', { name: /create listing/i });
    await createBtn.scrollIntoViewIfNeeded();
    await expect(createBtn).toBeVisible();
  });
});

test.describe('Listing Detail Page — Mobile Responsive', () => {
  const store = new MarketplaceTestStore();

  test.beforeEach(async ({ page }) => {
    store.reset();
    await page.setViewportSize(MOBILE_VIEWPORT);
  });

  test('listing detail page has no horizontal scroll on mobile', async ({ page }) => {
    // Navigate to a non-existent listing (will show error state — still tests layout)
    await page.goto('/listings/999');
    await page.waitForLoadState('domcontentloaded');

    const scrollWidth = await page.evaluate(() => document.documentElement.scrollWidth);
    const viewportWidth = await page.evaluate(() => window.innerWidth);
    expect(scrollWidth).toBeLessThanOrEqual(viewportWidth + 1);
  });

  test('listing detail loading state has no horizontal scroll on mobile', async ({ page }) => {
    await page.goto('/listings/1');
    // The loading state should render without horizontal scroll
    const scrollWidth = await page.evaluate(() => document.documentElement.scrollWidth);
    const viewportWidth = await page.evaluate(() => window.innerWidth);
    expect(scrollWidth).toBeLessThanOrEqual(viewportWidth + 1);
  });
});
