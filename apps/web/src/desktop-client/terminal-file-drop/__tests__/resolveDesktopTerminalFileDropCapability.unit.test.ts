// @vitest-environment jsdom

import { DESKTOP_FILE_PATH_BRIDGE_KEY } from '@orchbee/desktop-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import { resolveDesktopTerminalFileDropCapability } from '../resolveDesktopTerminalFileDropCapability';

describe('resolveDesktopTerminalFileDropCapability', () => {
	it('adapts the narrow preload API without exposing it to shared terminal policy', async () => {
		const file = new File([], 'space name.txt');
		const getPathForFile = vi.fn<(file: File) => string>(() => '/tmp/space name.txt');
		const targetWindow = Object.assign(window, {
			[DESKTOP_FILE_PATH_BRIDGE_KEY]: { getPathForFile, platform: 'darwin' },
		});

		const capability = resolveDesktopTerminalFileDropCapability(targetWindow);

		await expect(capability?.prepare([file])).resolves.toMatchObject({
			kind: 'ready',
			text: '/tmp/space\\ name.txt',
		});
		expect(getPathForFile).toHaveBeenCalledExactlyOnceWith(file);
		Reflect.deleteProperty(targetWindow, DESKTOP_FILE_PATH_BRIDGE_KEY);
	});

	it('rejects malformed bridge values and exposes no capability on unsupported platforms', () => {
		const missing = Object.assign(window, {
			[DESKTOP_FILE_PATH_BRIDGE_KEY]: { platform: 'darwin' },
		});
		expect(resolveDesktopTerminalFileDropCapability(missing)).toBeUndefined();

		const windows = Object.assign(window, {
			[DESKTOP_FILE_PATH_BRIDGE_KEY]: {
				getPathForFile: () => 'C:\\Users\\person\\file.txt',
				platform: 'win32',
			},
		});
		expect(resolveDesktopTerminalFileDropCapability(windows)).toBeUndefined();
		Reflect.deleteProperty(windows, DESKTOP_FILE_PATH_BRIDGE_KEY);
	});
});
