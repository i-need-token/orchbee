import { defineConfig } from '@rsbuild/core';
import { pluginReact } from '@rsbuild/plugin-react';
import { pluginSourceBuild } from '@rsbuild/plugin-source-build';

export default defineConfig(({ command }) => ({
	...(command === 'dev'
		? { tools: { rspack: { resolve: { conditionNames: ['@orchbee/source', '...'] } } } }
		: {}),
	plugins: [
		pluginReact(),
		...(command === 'dev' ? [pluginSourceBuild({ sourceField: '@orchbee/source' })] : []),
	],
	output: { cssModules: { localIdentName: 'ob-[name]-[local]-[hash:base64:5]' } },
	environments: {
		web: {
			source: { entry: { index: './src/composition/entry.tsx' } },
			html: {
				title: 'Orchbee',
				scriptLoading: 'blocking',
				inject: 'head',
				meta: { viewport: 'width=device-width, initial-scale=1', 'color-scheme': 'dark light' },
			},
			output: {
				distPath: { root: 'dist' },
				// Brand files and the README animations are published as they are; public/ adds the
				// re-encoded stills and the Cloudflare headers file.
				copy: [
					{ from: '../../resources/icon.svg', to: 'icon.svg' },
					{ from: '../../resources/brand/ico/favicon.ico', to: 'favicon.ico' },
					{
						from: '../../resources/brand/png/square/orchbee-icon-square-180.png',
						to: 'apple-touch-icon.png',
					},
					{ from: '../../resources/brand/social/orchbee-og-1200x630-dark.png', to: 'og.png' },
					{ from: '../../resources/readme/multiplex.gif', to: 'pictures/multiplex.gif' },
					{ from: '../../resources/readme/agents-cli.gif', to: 'pictures/agents-cli.gif' },
				],
			},
		},
		...(command === 'build'
			? {
					prerender: {
						source: { entry: { render: './src/index.ts' } },
						output: {
							target: 'node',
							distPath: { root: 'dist/.prerender' },
							module: true,
							filename: { js: '[name].mjs' },
							minify: false,
						},
						tools: { rspack: { output: { library: { type: 'module' } } } },
					},
				}
			: {}),
	},
	server: {
		host: '127.0.0.1',
		port: 3100,
		strictPort: true,
		htmlFallback: command === 'dev' ? 'index' : false,
	},
}));
