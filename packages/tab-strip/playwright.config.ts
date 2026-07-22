import { defineConfig } from '@playwright/test';

const installedBrowser = '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome';
const executablePath =
	process.env.TAB_STRIP_BROWSER_EXECUTABLE ??
	(process.platform === 'darwin' ? installedBrowser : undefined);

export default defineConfig({
	testDir: './system-tests',
	fullyParallel: false,
	workers: 1,
	retries: 0,
	timeout: 20_000,
	expect: { timeout: 5_000 },
	use: {
		baseURL: 'http://127.0.0.1:5198',
		browserName: 'chromium',
		colorScheme: 'light',
		deviceScaleFactor: 1,
		headless: true,
		locale: 'en-US',
		reducedMotion: 'reduce',
		screenshot: 'only-on-failure',
		trace: 'retain-on-failure',
		viewport: { width: 1440, height: 900 },
		launchOptions: executablePath ? { executablePath } : undefined,
	},
	webServer: {
		command: 'pnpm exec rsbuild dev --config rsbuild.system.config.ts',
		url: 'http://127.0.0.1:5198',
		reuseExistingServer: false,
		timeout: 30_000,
	},
});
