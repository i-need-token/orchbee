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
			index: './src/composition/entry.ts',
		},
	},
	html: {
		meta: {
			charset: { charset: 'utf-8' },
			'content-security-policy': {
				content:
					command === 'dev'
						? "default-src 'self'; script-src 'self' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'self' ws: http:"
						: "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'self' ws://127.0.0.1:*",
				'http-equiv': 'Content-Security-Policy',
			},
			viewport: 'width=device-width, initial-scale=1.0',
		},
		title: 'Orchbee Workbench',
	},
	server: {
		host: '127.0.0.1',
		port: 3000,
		strictPort: true,
	},
}));
