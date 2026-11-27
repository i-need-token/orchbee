import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { useLayoutEffect, useState, type RefObject } from 'react';

import type { CodeEditorViewStates } from '../../CodeEditorViewStates';
import { ImageViewer, type ImageViewerViewState } from '../../ImageViewer';

interface ImageViewerOptions {
	/** The scroll container the viewer zooms in. */
	readonly element: RefObject<HTMLDivElement | null>;
	readonly image: RefObject<HTMLImageElement | null>;
	readonly platform: KeyboardShortcutPlatform;
	/** The `data:` URL to show, or null while there is none. */
	readonly source: string | null;
	/** The key the shown file's view state is kept under. */
	readonly viewStateKey: string;
	readonly viewStates: CodeEditorViewStates<ImageViewerViewState>;
}

/**
 * VS Code's image viewer over the preview's elements while the preview is mounted. It shows each
 * source with the file's saved zoom and scroll, and saves them when the source changes or the
 * preview unmounts. Answers whether the shown source failed to decode as an image.
 */
export function useImageViewer({
	element,
	image,
	platform,
	source,
	viewStateKey,
	viewStates,
}: ImageViewerOptions): boolean {
	const [viewer, setViewer] = useState<ImageViewer | null>(null);
	const [undecodable, setUndecodable] = useState<string | null>(null);

	useLayoutEffect(() => {
		const container = element.current;
		const shown = image.current;
		if (container === null || shown === null) return undefined;
		const created = new ImageViewer(container, shown, platform);
		setViewer(created);
		return () => created.dispose();
	}, [element, image, platform]);

	// A layout effect, so the cleanup reads the scroll position before the viewer leaves the page.
	useLayoutEffect(() => {
		if (viewer === null || source === null) return undefined;
		let current = true;
		void viewer.setSource(source, viewStates.get(viewStateKey)).then((decoded) => {
			if (current) setUndecodable(decoded ? null : source);
		});
		return () => {
			current = false;
			viewer.cancelPendingSource();
			const shown = viewer.getViewState();
			if (shown !== undefined) viewStates.set(viewStateKey, shown);
		};
	}, [source, viewStateKey, viewStates, viewer]);

	return source !== null && undecodable === source;
}
