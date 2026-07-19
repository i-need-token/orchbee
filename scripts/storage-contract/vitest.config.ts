import { defineConfig } from 'vitest/config';

export default defineConfig({
	root: import.meta.dirname,
	test: {
		include: ['__tests__/*.unit.test.ts'],
	},
});
