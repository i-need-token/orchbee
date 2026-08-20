import { defineConfig } from '@playwright/test';

export default defineConfig({
	testDir: './system-tests',
	fullyParallel: false,
	workers: 1,
	retries: 0,
	timeout: 30_000,
	expect: { timeout: 10_000 },
	globalSetup: './scripts/require-manual-native-window-test-approval.cjs',
	grep: /@manual-native-window/u,
	outputDir: '../../test-results/desktop-manual-native-window',
});
