import { availableParallelism } from 'node:os';

import { defineConfig } from 'vitest/config';

export default defineConfig({
	root: import.meta.dirname,
	test: {
		// Hook fixtures launch real processes. Bound Windows contention without extending their
		// deadlines or weakening any assertions.
		...(process.platform === 'win32' ? { maxWorkers: Math.min(4, availableParallelism()) } : {}),
		environment: 'node',
		include: ['__tests__/*.unit.test.ts'],
	},
});
