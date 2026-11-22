import {
	DESKTOP_FILE_DROP_WINDOW_MESSAGE,
	DesktopFileDropWindowMessageSchema,
} from '@orchbee/desktop-client-protocol';

import { formatDroppedPaths } from './formatDroppedPaths';

interface DesktopFileDropWindow {
	addEventListener(
		type: 'drop',
		listener: (event: DragEvent) => void,
		options: { readonly capture: true },
	): void;
	readonly location: Pick<Location, 'origin'>;
	postMessage(message: unknown, targetOrigin: string): void;
}

/** A dropped file's path on disk; empty for one that has none. */
type DroppedFilePath = (file: File) => string;

/**
 * Posts the paths of each drop of files to the page, which pastes them into the terminal it was
 * dropped on. The page sees a drop only where it accepted the drag, and only the preload can read a
 * dropped file's path, so this listens before the page's own listeners do.
 */
export function installDesktopFileDropBridge(
	targetWindow: DesktopFileDropWindow,
	pathOf: DroppedFilePath,
	platform: NodeJS.Platform,
): void {
	targetWindow.addEventListener(
		'drop',
		(event) => {
			const files = event.dataTransfer?.files;
			if (files === undefined || files.length === 0) return;
			const paths = Array.from(files, pathOf).filter((path) => path !== '');
			if (paths.length === 0) return;
			const message = DesktopFileDropWindowMessageSchema.safeParse({
				text: formatDroppedPaths(paths, platform),
				type: DESKTOP_FILE_DROP_WINDOW_MESSAGE,
			});
			if (!message.success) return;
			targetWindow.postMessage(
				message.data,
				targetWindow.location.origin === 'null' ? '*' : targetWindow.location.origin,
			);
		},
		{ capture: true },
	);
}
