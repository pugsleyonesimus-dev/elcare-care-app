import { Page, expect } from '@playwright/test';
import { mockFreighter, TEST_PUBLIC_KEY } from '../freighter-mock';

export async function connectFreighterWallet(
  page: Page,
  publicKey: string = TEST_PUBLIC_KEY
) {
  await mockFreighter(page, { publicKey });
  await page.goto('/');
  await expect(page.getByText('ELCARE-HUB').first()).toBeVisible({ timeout: 30_000 });

  const nav = page.locator('nav');
  const shortKey = `${publicKey.slice(0, 4)}...${publicKey.slice(-4)}`;

  if (!(await page.getByText(shortKey).isVisible().catch(() => false))) {
    await nav.getByRole('button', { name: 'Connect Wallet', exact: true }).click();
    await page.getByRole('button', { name: /Freighter/i }).click();
  }

  await expect(
    page.getByTestId('wallet-connected').or(page.getByText(shortKey))
  ).toBeVisible({ timeout: 20_000 });
}

export async function openNewListingTab(page: Page) {
  await page.goto('/dashboard');
  await page.getByRole('button', { name: /new listing/i }).click();
  await expect(page.getByText('List Your Artwork')).toBeVisible();
}
