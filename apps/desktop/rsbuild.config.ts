import { createRequire } from 'node:module';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { defineConfig, type RsbuildPlugin } from '@rsbuild/core';
import { pluginSourceBuild } from '@rsbuild/plugin-source-build';

import type { DesktopDevelopmentRuntime, DesktopDevelopmentRuntimeLogger } from './src/development';
import { buildPosixHostEndpoint } from './scripts/build-posix-host-endpoint.mjs';
import { buildSemanticCommandEndpoint } from './scripts/build-semantic-command-endpoint.mjs';
import { prepareDesktopDevelopmentExecutable } from './scripts/prepare-desktop-development-application.mjs';

const desktopRoot = path.dirname(fileURLToPath(import.meta.url));

export default defineConfig(({ command }) => {
	let developmentRuntime: DesktopDevelopmentRuntime | undefined;
	const rendererUrl = process.env['ORCHBEE_DESKTOP_RENDERER_URL'] ?? 'http://127.0.0.1:3000/';
	const gatewayEntryPath =
		process.env['ORCHBEE_GATEWAY_ENTRY_PATH'] ??
		path.resolve(desktopRoot, '..', 'gateway', 'dist', 'main', 'gateway.cjs');
	const serverEntryPath =
		process.env['ORCHBEE_SERVER_ENTRY_PATH'] ??
		path.resolve(desktopRoot, '..', 'server', 'dist', 'main', 'server.cjs');
	const webRendererRoot =
		process.env['ORCHBEE_WEB_RENDERER_ROOT'] ?? path.resolve(desktopRoot, '..', 'web', 'dist');
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
				const sshEnvironmentRelease =
					process.platform === 'win32'
						? undefined
						: await (
								await import('./scripts/development-ssh-environment-release.mjs')
							).prepareDevelopmentSshEnvironmentRelease({
								outputRoot: path.join(desktopRoot, 'out', 'development-ssh-environment'),
								sourceRoot,
							});
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
						gatewayEntryPath,
						rendererUrl,
						serverEntryPath,
						sourceRoot,
						...(sshEnvironmentRelease === undefined
							? {}
							: {
									sshEnvironmentRelease: {
										releaseDirectoryPath: sshEnvironmentRelease.releaseDirectoryPath,
										trustDescriptorFilePath: sshEnvironmentRelease.trustDescriptorFilePath,
									},
								}),
						terminalHostRoot: path.resolve(desktopRoot, '..', 'terminal-host'),
						webRendererRoot,
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
				await Promise.all([buildPosixHostEndpoint(), buildSemanticCommandEndpoint()]);
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
					path.join(desktopRoot, 'scripts', 'build-semantic-command-endpoint.mjs'),
					path.join(desktopRoot, 'scripts', 'materialize-darwin-application-icon.mjs'),
					path.join(desktopRoot, 'scripts', 'prepare-desktop-development-application.mjs'),
					path.join(desktopRoot, 'src', 'distribution'),
					path.join(desktopRoot, 'src', 'native', 'posix-host-endpoint'),
					path.join(desktopRoot, 'src', 'native', 'semantic-command-endpoint'),
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
						'ssh-askpass-helper': './src/main/ssh-environment/ssh-askpass-helper.ts',
						'semantic-command-local-endpoint-worker':
							'./src/main/semantic-command/semantic-command-local-endpoint-worker.ts',
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
