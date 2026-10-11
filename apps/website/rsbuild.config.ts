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
				copy: [{ from: '../../resources/icon.svg', to: 'icon.svg' }],
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
