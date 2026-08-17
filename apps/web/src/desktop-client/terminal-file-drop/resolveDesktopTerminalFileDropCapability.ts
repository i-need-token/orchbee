import { DESKTOP_FILE_PATH_BRIDGE_KEY } from '@orchbee/desktop-client-protocol';

import {
	createTerminalFileDropCapability,
	type TerminalFileDropCapability,
} from '../../terminal-file-drop';

interface DesktopRendererFilePathBridge {
	readonly getPathForFile: (file: File) => unknown;
	readonly platform: unknown;
}

export function resolveDesktopTerminalFileDropCapability(
	targetWindow: Window,
): TerminalFileDropCapability | undefined {
	const candidate: unknown = Reflect.get(targetWindow, DESKTOP_FILE_PATH_BRIDGE_KEY);
	if (!isDesktopRendererFilePathBridge(candidate)) return undefined;
	return createTerminalFileDropCapability({
		getPathForFile: (file): unknown => Reflect.apply(candidate.getPathForFile, candidate, [file]),
		platform: candidate.platform,
	});
}

function isDesktopRendererFilePathBridge(value: unknown): value is DesktopRendererFilePathBridge {
	return (
		typeof value === 'object' &&
		value !== null &&
		'getPathForFile' in value &&
		typeof value.getPathForFile === 'function' &&
		'platform' in value
	);
}
