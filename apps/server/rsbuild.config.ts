import { defineConfig } from '@rsbuild/core';

export default defineConfig({
	output: {
		distPath: { root: 'dist/main' },
		filename: { js: '[name].cjs' },
		module: false,
		target: 'node',
	},
	source: {
		entry: { server: './src/composition/entry.ts' },
	},
	splitChunks: false,
	tools: {
		rspack(config) {
			config.target = 'node';
		},
	},
});
