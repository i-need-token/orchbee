import { availableParallelism } from 'node:os';
import { defineConfig } from 'vitest/config';

const availableWorkerCount = availableParallelism();
const maxWorkers = Math.min(4, availableWorkerCount);

process.stdout.write(
	`[workbench-vitest] availableParallelism=${availableWorkerCount} configuredMaxWorkers=${maxWorkers}\n`,
);

export default defineConfig({
	test: {
		include: ['src/**/__tests__/*.unit.test.{ts,tsx}'],
		maxWorkers,
	},
});
