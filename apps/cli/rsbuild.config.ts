import { defineConfig } from '@rsbuild/core';

export default defineConfig({
	output: {
		distPath: { root: 'dist/main' },
		filename: { js: '[name].cjs' },
		module: false,
		target: 'node',
	},
	source: {
		entry: {
			'cli-core': './src/index.ts',
			// Cross-application artifact contract: search for `semantic-command-cli-entry-name` before changing this value.
			'orchbee-cli': './src/process/entry.ts',
		},
	},
	splitChunks: false,
	tools: {
		rspack(config) {
			config.target = 'node';
			config.module ??= {};
			config.module.rules ??= [];
			config.module.rules.push({ resourceQuery: /raw/u, type: 'asset/source' });
		},
	},
});
