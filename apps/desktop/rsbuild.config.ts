import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { defineConfig, type RsbuildPlugin } from '@rsbuild/core';
import { pluginSourceBuild } from '@rsbuild/plugin-source-build';

import type { DesktopDevelopmentRuntime, DesktopDevelopmentRuntimeLogger } from './src/development';

import { prepareDesktopDevelopmentExecutable } from './scripts/prepare-desktop-development-application.mjs';

const desktopRoot = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig(({ command }) => {
	let developmentRuntime: DesktopDevelopmentRuntime | undefined;
	const rendererUrl = process.env['ORCHBEE_DESKTOP_RENDERER_URL'] ?? 'http://127.0.0.1:3000/';

	const sourceRoot = path.resolve(desktopRoot, '..', '..');
	const developmentRuntimePlugin: RsbuildPlugin = {
		name: 'orchbee-desktop-development-runtime',
		apply: 'serve',
		setup(api) {
			const logger: DesktopDevelopmentRuntimeLogger = {
				error: (message) => api.logger.error(message),
				info: (message) => api.logger.info(message),
				warn: (message) => api.logger.warn(message),
			};
			api.onAfterStartDevServer(async () => {
				if (developmentRuntime !== undefined) return;
				const sourceElectronExecutable: unknown = createRequire(import.meta.url)('electron');
				if (typeof sourceElectronExecutable !== 'string') {
					throw new Error('Electron development executable path is unavailable.');
				}
				// Configuration inspection runs before workspace build artifacts exist.
				const { DesktopDevelopmentRuntime, createNodeDesktopDevelopmentRuntimeDependencies } =
					await import('./src/development');
				const electronExecutable = await prepareDesktopDevelopmentExecutable({
					electronExecutable: sourceElectronExecutable,
					environment: process.env,
					iconDirectory: path.join(sourceRoot, 'resources', 'brand', 'icns'),
					outputRoot: path.join(desktopRoot, 'out', 'development-runtime'),
					sourceRoot,
				});
				const runtime = new DesktopDevelopmentRuntime(
					{
						desktopEntryPath: path.join(desktopRoot, 'dist', 'main', 'main.cjs'),
						electronExecutable,
						environment: process.env,
						rendererUrl,
						sourceRoot,
					},
					createNodeDesktopDevelopmentRuntimeDependencies(logger),
					logger,
				);
				developmentRuntime = runtime;
				try {
					await runtime.start();
				} catch (error: unknown) {
					if (developmentRuntime === runtime) developmentRuntime = undefined;
					throw error;
				}
			});
			api.onAfterDevCompile(({ isFirstCompile, stats }) => {
				if (isFirstCompile || stats.hasErrors()) return;
				developmentRuntime?.requestDesktopRestart();
			});
			api.onCloseDevServer(async () => {
				const runtime = developmentRuntime;
				developmentRuntime = undefined;
				await runtime?.stop();
			});
		},
	};

	return {
		plugins:
			command === 'dev'
				? [pluginSourceBuild({ sourceField: '@orchbee/source' }), developmentRuntimePlugin]
				: [],
		dev: {
			watchFiles: {
				paths: [
					path.join(desktopRoot, 'scripts', 'prepare-desktop-development-application.mjs'),
					path.join(desktopRoot, 'src', 'distribution'),
					path.join(sourceRoot, 'resources', 'brand', 'icns'),
				],
				type: 'reload-server',
			},
		},
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
