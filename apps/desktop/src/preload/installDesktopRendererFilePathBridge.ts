import { DESKTOP_FILE_PATH_BRIDGE_KEY } from '@orchbee/desktop-client-protocol';

export interface DesktopRendererFilePathContextBridge {
	exposeInMainWorld(apiKey: string, api: DesktopRendererFilePathBridge): void;
}

export interface DesktopRendererFilePathWebUtils {
	getPathForFile(file: File): string;
}

export interface DesktopRendererFilePathBridge {
	readonly platform: NodeJS.Platform;
	getPathForFile(file: File): string;
}

export function installDesktopRendererFilePathBridge(
	contextBridge: DesktopRendererFilePathContextBridge,
	webUtils: DesktopRendererFilePathWebUtils,
	platform: NodeJS.Platform,
): void {
	contextBridge.exposeInMainWorld(DESKTOP_FILE_PATH_BRIDGE_KEY, {
		getPathForFile: (file) => webUtils.getPathForFile(file),
		platform,
	});
}
