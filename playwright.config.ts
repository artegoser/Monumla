import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
	testDir: './tests/browser',
	timeout: 45_000,
	fullyParallel: false,
	workers: 1,
	reporter: 'list',
	use: {
		baseURL: 'http://127.0.0.1:4173',
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
			'NODE_ENV=production node node_modules/vite/bin/vite.js preview --host 127.0.0.1 --port 4173 --strictPort',
		url: 'http://127.0.0.1:4173',
		reuseExistingServer: !process.env.CI,
		timeout: 30_000
	}
});
