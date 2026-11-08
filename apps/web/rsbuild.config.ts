import { defineConfig } from '@rsbuild/core';
import { pluginReact } from '@rsbuild/plugin-react';
import { pluginSourceBuild } from '@rsbuild/plugin-source-build';
import { pluginTailwindcss } from '@rsbuild/plugin-tailwindcss';

import { developmentServerPorts } from '../../scripts/development-server-ports.mjs';

export default defineConfig(({ command }) => ({
	...(command === 'dev' ? { dev: { hmr: false, liveReload: true } } : {}),
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
						: "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'self' ws:",
				'http-equiv': 'Content-Security-Policy',
			},
			viewport:
				'width=device-width, initial-scale=1.0, viewport-fit=cover, interactive-widget=resizes-content',
		},
		// What the TUI shows when no title policy names it; a Server-backed window is renamed once listening.
		title: 'Orchbee',
	},
	server: {
		host: '127.0.0.1',
		// Derived from the checkout, so two checkouts serve their renderers side by side.
		port: developmentServerPorts().web,
		strictPort: true,
	},
}));
