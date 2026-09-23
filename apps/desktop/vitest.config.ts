import { availableParallelism } from 'node:os';

import { defineConfig } from 'vitest/config';

export default defineConfig({
	test: {
		// Git and hook fixtures launch real processes. Bound Windows contention
		// without extending their deadlines or weakening any assertions.
		...(process.platform === 'win32' ? { maxWorkers: Math.min(4, availableParallelism()) } : {}),
		environment: 'node',
		include: ['src/**/__tests__/*.unit.test.{ts,tsx}'],
	},
});
