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
		// `@orchbee/terminal` is publishable, and its CommonJS build requires this package.
		{ format: 'cjs', syntax: 'es2022' },
	],
	source: {
		tsconfigPath: './tsconfig.build.json',
	},
	output: {
		target: 'web',
		sourceMap: true,
	},
});
