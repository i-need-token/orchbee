import { configDefaults, defineConfig } from 'vitest/config';

export default defineConfig({
	test: {
		environment: 'jsdom',
		exclude: [...configDefaults.exclude, 'system-tests/**'],
		setupFiles: ['./test/setup.ts'],
	},
});
