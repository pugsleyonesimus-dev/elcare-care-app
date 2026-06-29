import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
    testDir: './tests/e2e',
    fullyParallel: true,
    forbidOnly: !!process.env.CI,
    retries: process.env.CI ? 2 : 0,
    workers: process.env.CI ? 1 : undefined,
    reporter: process.env.CI ? [['list'], ['html', { open: 'never' }]] : 'list',
    use: {
        baseURL: 'http://localhost:3000',
        trace: 'on-first-retry',
        screenshot: 'only-on-failure',
        headless: true,
    },
    projects: [
        {
            name: 'chromium',
            use: { ...devices['Desktop Chrome'] },
        },
    ],
    webServer: {
        command: 'npm run dev:e2e',
        url: 'http://localhost:3000',
        reuseExistingServer: !process.env.CI,
        timeout: 120 * 1000,
        env: {
            NEXT_PUBLIC_E2E_MOCK_CHAIN: 'true',
            NEXT_PUBLIC_CONTRACT_ID: process.env.NEXT_PUBLIC_CONTRACT_ID ?? 'CE2ECONTRACTPLACEHOLDER00000000000000000000001',
            NEXT_PUBLIC_LAUNCHPAD_CONTRACT_ID:
                process.env.NEXT_PUBLIC_LAUNCHPAD_CONTRACT_ID ?? 'CE2ELAUNCHPADPLACEHOLDER000000000000000000001',
            PINATA_JWT: process.env.PINATA_JWT ?? 'e2e-test-pinata-jwt',
        },
    },
});
