import { defineConfig } from '@rslib/core';
import { pluginReact } from '@rsbuild/plugin-react';
import { pluginTailwindcss } from '@rsbuild/plugin-tailwindcss';

const componentsEntry = { 'components/index': './src/components/index.ts' };

export default defineConfig({
	plugins: [pluginReact(), pluginTailwindcss()],
	lib: [
		{
			format: 'esm',
			syntax: 'es2022',
			dts: {
				alias: {
					'#components/*': './src/components/*',
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
			// The style entry shares its CSS Modules with the components, and this output would move
			// them into a chunk the components load through a runtime chunk, so it leaves the
			// stylesheet to the unsplit output below.
			source: { entry: componentsEntry },
		},
		// Writes `dist/styles.css`, which `./styles.css` names, with every module the style entry imports.
		{
			format: 'cjs',
			syntax: 'es2022',
			source: { entry: { ...componentsEntry, styles: './src/styles.ts' } },
		},
	],
	source: {
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
