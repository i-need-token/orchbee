import { DESKTOP_FILE_PATH_BRIDGE_KEY } from '@orchbee/desktop-client-protocol';
import { describe, expect, it, vi } from 'vitest';

import {
	installDesktopRendererFilePathBridge,
	type DesktopRendererFilePathBridge,
	type DesktopRendererFilePathContextBridge,
	type DesktopRendererFilePathWebUtils,
} from '../installDesktopRendererFilePathBridge';

describe('installDesktopRendererFilePathBridge', () => {
	it('exposes only platform and File-scoped backing-path lookup', () => {
		let exposed: DesktopRendererFilePathBridge | undefined;
		const contextBridge: DesktopRendererFilePathContextBridge = {
			exposeInMainWorld: vi.fn<DesktopRendererFilePathContextBridge['exposeInMainWorld']>(
				(key, api) => {
					expect(key).toBe(DESKTOP_FILE_PATH_BRIDGE_KEY);
					exposed = api;
				},
			),
		};
		const webUtils: DesktopRendererFilePathWebUtils = {
			getPathForFile: vi.fn<DesktopRendererFilePathWebUtils['getPathForFile']>(
				() => '/tmp/user-selected.txt',
			),
		};
		const file = { name: 'user-selected.txt' } as File;

		installDesktopRendererFilePathBridge(contextBridge, webUtils, 'darwin');

		expect(Object.keys(exposed ?? {}).sort()).toEqual(['getPathForFile', 'platform']);
		expect(exposed?.platform).toBe('darwin');
		expect(exposed?.getPathForFile(file)).toBe('/tmp/user-selected.txt');
		expect(webUtils.getPathForFile).toHaveBeenCalledExactlyOnceWith(file);
	});
});
