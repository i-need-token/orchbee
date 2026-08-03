import { spawn, type ChildProcess } from 'node:child_process';
import { createRequire } from 'node:module';
import path from 'node:path';

import { defineConfig, type RsbuildPlugin } from '@rsbuild/core';

const electronExecutable: unknown = createRequire(import.meta.url)('electron');
if (typeof electronExecutable !== 'string') {
	throw new Error('Electron development executable path is unavailable.');
}

export default defineConfig(() => {
	let desktopProcess: ChildProcess | undefined;
	const rendererUrl = process.env['ORCHBEE_DESKTOP_RENDERER_URL'] ?? 'http://127.0.0.1:3000/';
	const gatewayEntryPath =
		process.env['ORCHBEE_GATEWAY_ENTRY_PATH'] ??
		path.resolve('..', 'gateway', 'dist', 'main', 'gateway.cjs');
	const launchDesktopPlugin: RsbuildPlugin = {
		name: 'orchbee-desktop-development-launcher',
		apply: 'serve',
		setup(api) {
			api.onAfterStartDevServer(() => {
				if (desktopProcess !== undefined) return;
				desktopProcess = spawn(electronExecutable, [path.resolve('dist/main/main.cjs')], {
					env: {
						...process.env,
						ORCHBEE_DESKTOP_GATEWAY: process.env['ORCHBEE_DESKTOP_GATEWAY'] ?? '1',
						ORCHBEE_DESKTOP_RENDERER_URL: rendererUrl,
						ORCHBEE_GATEWAY_ENTRY_PATH: gatewayEntryPath,
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
		plugins: [launchDesktopPlugin],
		environments: {
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
