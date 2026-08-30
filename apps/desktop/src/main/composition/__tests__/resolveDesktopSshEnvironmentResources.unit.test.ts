import { describe, expect, it } from 'vitest';

import { resolveDesktopSshEnvironmentResources } from '../resolveDesktopSshEnvironmentResources';

describe('Desktop SSH Environment resource composition', () => {
	it('keeps unconfigured development inert while using the built AskPass helper', () => {
		expect(
			resolveDesktopSshEnvironmentResources({
				mainDirectoryPath: '/repo/apps/desktop/dist/main',
				packaged: false,
				runtimeExecutablePath: '/Applications/Electron.app/Contents/MacOS/Electron',
			}),
		).toEqual({
			askPassHelperEntryPath: '/repo/apps/desktop/dist/main/ssh-askpass-helper.cjs',
			askPassRuntimeExecutablePath: '/Applications/Electron.app/Contents/MacOS/Electron',
			release: null,
		});
	});

	it('binds source development to its prepared release and trust descriptor', () => {
		expect(
			resolveDesktopSshEnvironmentResources({
				mainDirectoryPath: '/repo/apps/desktop/dist/main',
				packaged: false,
				release: {
					releaseDirectoryPath: '/repo/apps/desktop/out/development/release',
					trustDescriptorFilePath: '/repo/apps/desktop/out/development/trust.json',
				},
				runtimeExecutablePath: '/Applications/Electron.app/Contents/MacOS/Electron',
			}),
		).toEqual({
			askPassHelperEntryPath: '/repo/apps/desktop/dist/main/ssh-askpass-helper.cjs',
			askPassRuntimeExecutablePath: '/Applications/Electron.app/Contents/MacOS/Electron',
			release: {
				releaseDirectoryPath: '/repo/apps/desktop/out/development/release',
				trustDescriptorFilePath: '/repo/apps/desktop/out/development/trust.json',
			},
		});
	});

	it('binds packaged artifacts to Resources and trust to the app-asar Main directory', () => {
		expect(
			resolveDesktopSshEnvironmentResources({
				mainDirectoryPath: '/Applications/Orchbee.app/Contents/Resources/app.asar/dist/main',
				packaged: true,
				releaseAvailable: true,
				resourcesPath: '/Applications/Orchbee.app/Contents/Resources',
				runtimeExecutablePath: '/Applications/Orchbee.app/Contents/MacOS/Orchbee',
			}),
		).toEqual({
			askPassHelperEntryPath: '/Applications/Orchbee.app/Contents/Resources/ssh-askpass-helper.cjs',
			askPassRuntimeExecutablePath: '/Applications/Orchbee.app/Contents/MacOS/Orchbee',
			release: {
				releaseDirectoryPath:
					'/Applications/Orchbee.app/Contents/Resources/ssh-environment-release-v2',
				trustDescriptorFilePath:
					'/Applications/Orchbee.app/Contents/Resources/app.asar/dist/main/ssh-environment-release-trust-v2.json',
			},
		});
	});

	it('keeps a release-free packaged profile inert', () => {
		expect(
			resolveDesktopSshEnvironmentResources({
				mainDirectoryPath: '/Applications/Orchbee Local.app/Contents/Resources/app.asar/dist/main',
				packaged: true,
				releaseAvailable: false,
				resourcesPath: '/Applications/Orchbee Local.app/Contents/Resources',
				runtimeExecutablePath: '/Applications/Orchbee Local.app/Contents/MacOS/Orchbee Local',
			}),
		).toEqual({
			askPassHelperEntryPath:
				'/Applications/Orchbee Local.app/Contents/Resources/ssh-askpass-helper.cjs',
			askPassRuntimeExecutablePath: '/Applications/Orchbee Local.app/Contents/MacOS/Orchbee Local',
			release: null,
		});
	});

	it('rejects relative executable, Main, and packaged Resources paths', () => {
		expect(() =>
			resolveDesktopSshEnvironmentResources({
				mainDirectoryPath: 'dist/main',
				packaged: false,
				runtimeExecutablePath: '/usr/bin/electron',
			}),
		).toThrow('absolute');
		expect(() =>
			resolveDesktopSshEnvironmentResources({
				mainDirectoryPath: '/app/dist/main',
				packaged: false,
				runtimeExecutablePath: 'electron',
			}),
		).toThrow('absolute');
		expect(() =>
			resolveDesktopSshEnvironmentResources({
				mainDirectoryPath: '/app/dist/main',
				packaged: true,
				releaseAvailable: true,
				resourcesPath: 'resources',
				runtimeExecutablePath: '/usr/bin/electron',
			}),
		).toThrow('absolute');
	});
});
