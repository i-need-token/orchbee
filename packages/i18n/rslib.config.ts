import { defineConfig } from '@rslib/core';
import { pluginReact } from '@rsbuild/plugin-react';

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
