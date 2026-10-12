import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { defineConfig, type RsbuildPlugin } from '@rsbuild/core';
import { pluginSourceBuild } from '@rsbuild/plugin-source-build';

import type { DesktopDevelopmentRuntime, DesktopDevelopmentRuntimeLogger } from './src/development';
import { buildPosixHostEndpoint } from './scripts/build-posix-host-endpoint.mjs';
import { buildSettingsBridge } from './scripts/build-settings-bridge.mjs';

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
					baseIconPath: path.join(sourceRoot, 'resources', 'icon.svg'),
					electronExecutable: sourceElectronExecutable,
					environment: process.env,
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
						terminalHostRoot: path.resolve(desktopRoot, '..', 'terminal-host'),
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
	const nativeEndpointPlugin: RsbuildPlugin = {
		name: 'orchbee-desktop-native-endpoints',
		apply: 'serve',
		setup(api) {
			api.onBeforeCreateCompiler(async () => {
				await buildPosixHostEndpoint();
				await buildSettingsBridge();
			});
		},
	};

	return {
		plugins:
			command === 'dev'
				? [
						pluginSourceBuild({ sourceField: '@orchbee/source' }),
						nativeEndpointPlugin,
						developmentRuntimePlugin,
					]
				: [],
		dev: {
			watchFiles: {
				paths: [
					path.join(desktopRoot, 'scripts', 'build-posix-host-endpoint.mjs'),
					path.join(desktopRoot, 'scripts', 'build-settings-bridge.mjs'),
					path.join(sourceRoot, 'apps', 'orchbee-settings', 'src'),
					path.join(sourceRoot, 'crates', 'local-site-runtime', 'src', 'user_preferences'),
					path.join(sourceRoot, 'crates', 'local-site-runtime', 'src', 'user_preferences.rs'),
					path.join(
						sourceRoot,
						'crates',
						'local-site-runtime',
						'src',
						'default_interactive_launch',
					),
					path.join(
						sourceRoot,
						'crates',
						'local-site-runtime',
						'src',
						'default_interactive_launch.rs',
					),
					path.join(sourceRoot, 'crates', 'workbench-protocol', 'src', 'user_preferences'),
					path.join(sourceRoot, 'crates', 'workbench-protocol', 'src', 'user_preferences.rs'),

					path.join(desktopRoot, 'scripts', 'materialize-darwin-application-icon.mjs'),
					path.join(desktopRoot, 'scripts', 'prepare-desktop-development-application.mjs'),
					path.join(desktopRoot, 'src', 'distribution'),
					path.join(desktopRoot, 'src', 'native', 'posix-host-endpoint'),

					path.join(sourceRoot, 'resources', 'icon.svg'),
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
				source: {
					entry: {
						main: './src/main/composition/entry.ts',
					},
				},
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
