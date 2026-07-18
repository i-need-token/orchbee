import { defineConfig } from '@rslib/core';

export default defineConfig({
	lib: [
		{
			format: 'esm',
			syntax: 'es2022',
			dts: true,
			redirect: {
				dts: {
					extension: true,
				},
			},
		},
		{ format: 'cjs', syntax: 'es2022' },
	],
	source: {
		entry: {
			index: './src/index.ts',
			'tailwind-compat': './src/tailwind-compat.css',
			tokens: './src/tokens.css',
		},
		tsconfigPath: './tsconfig.build.json',
	},
	output: {
		target: 'web',
		sourceMap: true,
	},
});
