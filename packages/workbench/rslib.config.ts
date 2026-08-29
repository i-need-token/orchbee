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
		entry: {
			index: './src/index.ts',
			'document/index': './src/workbench/document/index.ts',
			'semantic-commands/index': './src/workbench/semantic-commands/index.ts',
			'terminal-presentation/index': './src/terminal-presentation/index.ts',
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
