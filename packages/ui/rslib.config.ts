import { defineConfig } from '@rslib/core';
import { pluginReact } from '@rsbuild/plugin-react';
import { pluginTailwindcss } from '@rsbuild/plugin-tailwindcss';

export default defineConfig({
	plugins: [pluginReact(), pluginTailwindcss()],
	lib: [
		{
			format: 'esm',
			syntax: 'es2022',
			dts: {
				alias: {
					'#components/*': './src/components/*',
					'#hooks': './src/hooks/index.ts',
					'#hooks/*': './src/hooks/*',
					'#lib': './src/lib/index.ts',
					'#lib/*': './src/lib/*',
				},
			},
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
			'components/index': './src/components/index.ts',
			styles: './src/styles.css',
		},
		tsconfigPath: './tsconfig.build.json',
	},
	output: {
		target: 'web',
		sourceMap: true,
		cssModules: {
			localIdentName: 'ob-[local]-[hash:base64:5]',
		},
	},
});
