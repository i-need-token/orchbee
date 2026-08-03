import { defineConfig } from '@rsbuild/core';

export default defineConfig({
	output: {
		distPath: { root: 'dist/main' },
		externals: ['electron'],
		filename: { js: '[name].cjs' },
		module: false,
		target: 'node',
	},
	source: {
		entry: { gateway: './src/composition/entry.ts' },
	},
	splitChunks: false,
	tools: {
		rspack(config) {
			config.target = 'node';
		},
	},
});
