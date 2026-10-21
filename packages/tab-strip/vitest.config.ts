import { configDefaults, defineConfig } from 'vitest/config';

export default defineConfig({
	test: {
		environment: 'node',
		exclude: [...configDefaults.exclude, 'system-tests/**'],
		setupFiles: ['./test/setup.ts'],
	},
});
