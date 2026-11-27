import { defineConfig, type RsbuildPlugin } from '@rsbuild/core';
import { pluginReact } from '@rsbuild/plugin-react';
import { pluginSourceBuild } from '@rsbuild/plugin-source-build';
import { pluginTailwindcss } from '@rsbuild/plugin-tailwindcss';

import { developmentServerPorts } from '../../scripts/development-server-ports.mjs';

// Monaco's codicon stylesheet declares its own font under the product's `codicon` family and
// sizes every codicon on the page. Renamed, and limited to the roots Monaco themes, it leaves the
// product's icons on the product's font and sizes once the editor loads.
const MONACO_CODICON_REWRITES = [
	['font-family: "codicon";', 'font-family: "monaco-codicon";'],
	[
		".codicon[class*='codicon-'] {",
		":where(.monaco-editor, .monaco-diff-editor, .monaco-component) .codicon[class*='codicon-'] {",
	],
	['16px/1 codicon;', '16px/1 monaco-codicon;'],
] as const;

const monacoCodiconPlugin: RsbuildPlugin = {
	name: 'orchbee-web-monaco-codicon',
	setup(api) {
		api.transform(
			{
				order: 'pre',
				test: /[\\/]monaco-editor[\\/]esm[\\/]vs[\\/]base[\\/]browser[\\/]ui[\\/]codicons[\\/]codicon[\\/]codicon\.css$/u,
			},
			({ code }) =>
				MONACO_CODICON_REWRITES.reduce((rewritten, [from, to]) => {
					if (rewritten.split(from).length !== 2) {
						throw new Error(`Monaco's codicon stylesheet no longer has exactly one ${from}`);
					}
					return rewritten.replace(from, to);
				}, code),
		);
	},
};

export default defineConfig(({ command }) => ({
	...(command === 'dev' ? { dev: { hmr: false, liveReload: true } } : {}),
	plugins: [
		pluginReact(),
		monacoCodiconPlugin,
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
