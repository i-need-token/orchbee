import { describe, expect, it } from 'vitest';

import { resolveDesktopSshEnvironmentResources } from '../resolveDesktopSshEnvironmentResources';

describe('Desktop SSH Environment resource composition', () => {
	it('keeps release-free development inert while using the built AskPass helper', () => {
		expect(
			resolveDesktopSshEnvironmentResources({
				mainDirectoryPath: '/repo/apps/desktop/dist/main',
				packaged: false,
				runtimeExecutablePath: '/Applications/Electron.app/Contents/MacOS/Electron',
			}),
		).toEqual({
			askPassHelperEntryPath: '/repo/apps/desktop/dist/main/ssh-askpass-helper.cjs',
			askPassRuntimeExecutablePath: '/Applications/Electron.app/Contents/MacOS/Electron',
			runtimeRelease: null,
		});
	});

	it('binds packaged artifacts to Resources and trust to the app-asar Main directory', () => {
		expect(
			resolveDesktopSshEnvironmentResources({
				mainDirectoryPath: '/Applications/Orchbee.app/Contents/Resources/app.asar/dist/main',
				packaged: true,
				resourcesPath: '/Applications/Orchbee.app/Contents/Resources',
				runtimeExecutablePath: '/Applications/Orchbee.app/Contents/MacOS/Orchbee',
			}),
		).toEqual({
			askPassHelperEntryPath: '/Applications/Orchbee.app/Contents/Resources/ssh-askpass-helper.cjs',
			askPassRuntimeExecutablePath: '/Applications/Orchbee.app/Contents/MacOS/Orchbee',
			runtimeRelease: {
				releaseDirectoryPath:
					'/Applications/Orchbee.app/Contents/Resources/ssh-environment-runtime-release-v1',
				trustDescriptorFilePath:
					'/Applications/Orchbee.app/Contents/Resources/app.asar/dist/main/ssh-environment-runtime-release-trust-v1.json',
			},
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
				resourcesPath: 'resources',
				runtimeExecutablePath: '/usr/bin/electron',
			}),
		).toThrow('absolute');
	});
});
