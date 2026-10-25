import { defineConfig } from '@rslib/core';

export default defineConfig({
	lib: [{ format: 'esm', syntax: 'es2022', dts: false }],
	source: {
		entry: {
			'tailwind-compat': './src/tailwind-compat.css',
			tokens: './src/tokens.css',
		},
	},
	output: {
		target: 'web',
		sourceMap: true,
	},
});
