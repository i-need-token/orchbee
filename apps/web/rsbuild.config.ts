import { defineConfig } from '@rsbuild/core';
import { pluginReact } from '@rsbuild/plugin-react';
import { pluginSourceBuild } from '@rsbuild/plugin-source-build';
import { pluginTailwindcss } from '@rsbuild/plugin-tailwindcss';

export default defineConfig(({ command }) => ({
	plugins: [
		pluginReact(),
		...(command === 'dev'
			? [pluginTailwindcss(), pluginSourceBuild({ sourceField: '@orchbee/source' })]
			: []),
	],
	source: {
		entry: {
			index: './src/composition/entry.tsx',
		},
	},
	html: {
		title: 'Orchbee Workbench',
	},
	server: {
		host: '127.0.0.1',
		port: 3000,
		strictPort: true,
	},
}));
