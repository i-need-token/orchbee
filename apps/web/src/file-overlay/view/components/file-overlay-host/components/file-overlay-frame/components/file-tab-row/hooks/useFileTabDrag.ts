import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { useEffect, useLayoutEffect, useRef, type RefObject } from 'react';

import { followFileTabPress, type FileTabGesture } from '../utils/followFileTabPress';

interface FileTabDragOptions {
	/** The tab list, whose children are the row's tabs in its order. */
	readonly listReference: RefObject<HTMLElement | null>;
	/** Moves a tab to a place in the row, counted from 0, as the release of a drag asks. */
	readonly moveTab: (tabId: string, index: number) => void;
	/** The tabs' ids in the row's order, as one string; another order ends a drag that holds a tab. */
	readonly order: string;
	readonly platform: KeyboardShortcutPlatform;
	/** The row, which scrolls the list sideways. */
	readonly rowReference: RefObject<HTMLElement | null>;
	/** The tabs in the row's order. */
	readonly tabs: readonly { readonly id: string }[];
}

/**
 * Lets the pointer reorder the row's tabs by dragging one along it, with the squeeze motion of the
 * direct reorder vocabulary; the release runs the move. A touch scrolls the row instead, and a press
 * on a tab's button or a Control-press on a Mac never drags.
 */
export function useFileTabDrag({
	listReference,
	moveTab,
	order,
	platform,
	rowReference,
	tabs,
}: FileTabDragOptions): void {
	const gesture = useRef<FileTabGesture | null>(null);
	const latest = useRef({ moveTab, platform, tabs });
	useLayoutEffect(() => {
		latest.current = { moveTab, platform, tabs };
	});
	useLayoutEffect(() => {
		gesture.current?.rowChanged();
	}, [order]);
	useEffect(() => {
		const list = listReference.current;
		const row = rowReference.current;
		if (list === null || row === null) return undefined;
		const onPointerDown = (event: PointerEvent): void => {
			if (event.button !== 0 || !event.isPrimary) return;
			if (event.pointerType !== 'mouse' && event.pointerType !== 'pen') return;
			// A Control-press on a Mac asks for the tab's menu.
			if (event.ctrlKey && latest.current.platform === 'apple') return;
			const { target } = event;
			if (!(target instanceof Element)) return;
			if (target.closest('[data-slot="file-tab-action"]') !== null) return;
			const pressed = target.closest<HTMLElement>('[role="tab"]');
			if (pressed === null || pressed.parentElement !== list) return;
			if (gesture.current !== null) {
				if (!gesture.current.settling()) return;
				gesture.current.dispose();
			}
			let started: FileTabGesture | null = null;
			started = followFileTabPress({
				list,
				onEnd: () => {
					if (gesture.current === started) gesture.current = null;
				},
				onRelease: (from, to) => {
					const tab = latest.current.tabs[from];
					if (tab !== undefined) latest.current.moveTab(tab.id, to);
				},
				press: event,
				pressed,
				row,
			});
			gesture.current = started;
		};
		list.addEventListener('pointerdown', onPointerDown);
		return () => {
			list.removeEventListener('pointerdown', onPointerDown);
			gesture.current?.dispose();
			gesture.current = null;
		};
	}, [listReference, rowReference]);
}
