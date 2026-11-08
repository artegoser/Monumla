import { defineConfig, devices } from '@playwright/test';
import { resolve } from 'node:path';

export default defineConfig({
	testDir: './tests/browser',
	timeout: 45_000,
	fullyParallel: false,
	workers: 1,
	reporter: 'list',
	use: {
		baseURL: 'http://127.0.0.1:4173',
		actionTimeout: 10_000,
		trace: 'retain-on-failure',
		launchOptions: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE
			? { executablePath: process.env.PLAYWRIGHT_CHROMIUM_EXECUTABLE }
			: {}
	},
	projects: [
		{ name: 'desktop', use: { ...devices['Desktop Chrome'] } },
		{ name: 'mobile', use: { ...devices['Pixel 7'], defaultBrowserType: 'chromium' } }
	],
	webServer: {
		command:
			'pnpm db:migrate && NODE_ENV=production node node_modules/vite/bin/vite.js preview --host 127.0.0.1 --port 4173 --strictPort',
		env: {
			DATABASE_URL: `file:${resolve('test-results/browser.db')}`,
			ORIGIN: 'http://127.0.0.1:4173'
		},
		url: 'http://127.0.0.1:4173',
		reuseExistingServer: false,
		timeout: 30_000
	}
});
