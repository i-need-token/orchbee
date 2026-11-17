import { createRequire } from 'node:module';
import { defineConfig } from 'vitest/config';

// Electron downloads its binary the first time it is required. Vitest loads this file once in
// its main process before any test worker starts, so requiring Electron here keeps parallel
// workers from racing to extract it into a fresh checkout. Other tools that read this file skip it.
if (process.env['VITEST'] === 'true') createRequire(import.meta.url)('electron');

export default defineConfig({
	test: {
		environment: 'node',
		include: ['src/**/__tests__/*.unit.test.{ts,tsx}'],
	},
});
