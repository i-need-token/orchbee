import { pluginReact } from '@rsbuild/plugin-react';
import { defineConfig } from '@rslib/core';

export default defineConfig({
	plugins: [pluginReact()],
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
		// The ESM output splits the stylesheet into a shared chunk; this unsplit output is the one that
		// writes the single `dist/styles.css` the `./styles.css` export names.
		{ format: 'cjs', syntax: 'es2022' },
	],
	source: {
		entry: {
			index: './src/index.ts',
			'tab-strip/model/index': './src/tab-strip/model/index.ts',
			styles: './src/styles.ts',
		},
		tsconfigPath: './tsconfig.build.json',
	},
	output: {
		target: 'web',
		sourceMap: true,
	},
});
