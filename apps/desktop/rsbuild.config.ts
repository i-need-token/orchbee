import { spawn, type ChildProcess } from 'node:child_process';
import { createRequire } from 'node:module';
import path from 'node:path';

import { defineConfig, type RsbuildPlugin } from '@rsbuild/core';
import { pluginReact } from '@rsbuild/plugin-react';
import { pluginSourceBuild } from '@rsbuild/plugin-source-build';

const electronExecutable: unknown = createRequire(import.meta.url)('electron');
if (typeof electronExecutable !== 'string') {
	throw new Error('Electron development executable path is unavailable.');
}

export default defineConfig(({ command }) => {
	const development = command === 'dev';
	let desktopProcess: ChildProcess | undefined;
	const launchDesktopPlugin: RsbuildPlugin = {
		name: 'orchbee-desktop-development-launcher',
		apply: 'serve',
		setup(api) {
			api.onAfterStartDevServer(({ port }) => {
				if (desktopProcess !== undefined) return;
				desktopProcess = spawn(electronExecutable, [path.resolve('dist/main/main.cjs')], {
					env: {
						...process.env,
						ORCHBEE_DESKTOP_RENDERER_URL: `http://127.0.0.1:${port}`,
					},
					stdio: 'inherit',
				});
			});
			api.onCloseDevServer(() => {
				desktopProcess?.kill();
				desktopProcess = undefined;
			});
		},
	};

	return {
		plugins: [pluginReact(), launchDesktopPlugin],
		environments: {
			renderer: {
				plugins: development ? [pluginSourceBuild({ sourceField: '@orchbee/source' })] : [],
				source: {
					entry: { index: './src/renderer/composition/entry.ts' },
				},
				html: {
					meta: {
						charset: { charset: 'utf-8' },
						'content-security-policy': {
							content: development
								? "default-src 'self'; script-src 'self' 'unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'self' ws: http:"
								: "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; connect-src 'self'",
							'http-equiv': 'Content-Security-Policy',
						},
						viewport: 'width=device-width, initial-scale=1.0',
					},
					title: 'Orchbee',
				},
				output: {
					assetPrefix: './',
					distPath: 'dist/renderer',
					target: 'web',
				},
			},
			main: {
				dev: { hmr: false, liveReload: false, writeToDisk: true },
				output: {
					distPath: 'dist/main',
					externals: ['electron'],
					filename: { js: '[name].cjs' },
					module: false,
					target: 'node',
				},
				source: { entry: { main: './src/main/composition/entry.ts' } },
				splitChunks: false,
				tools: {
					rspack(config) {
						config.target = 'electron-main';
					},
				},
			},
			preload: {
				dev: { hmr: false, liveReload: false, writeToDisk: true },
				output: {
					distPath: 'dist/preload',
					externals: ['electron'],
					filename: { js: '[name].cjs' },
					module: false,
					target: 'node',
				},
				source: { entry: { preload: './src/preload/entry.ts' } },
				splitChunks: false,
				tools: {
					rspack(config) {
						config.target = 'electron-preload';
					},
				},
			},
		},
		server: {
			host: '127.0.0.1',
			port: 3010,
			strictPort: true,
		},
	};
});
