import { defineConfig } from '@playwright/test';

export default defineConfig({
	testDir: './system-tests',
	fullyParallel: false,
	workers: 1,
	retries: 0,
	timeout: 30_000,
	expect: { timeout: 10_000 },
	outputDir: '../../test-results/desktop',
});
