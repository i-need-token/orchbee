import { availableParallelism } from 'node:os';

import { defineConfig } from 'vitest/config';

export default defineConfig({
	test: {
		// Bound Windows process and filesystem contention alongside the other workspace suites.
		...(process.platform === 'win32' ? { maxWorkers: Math.min(4, availableParallelism()) } : {}),
		include: ['src/**/__tests__/*.unit.test.{ts,tsx}'],
	},
});
