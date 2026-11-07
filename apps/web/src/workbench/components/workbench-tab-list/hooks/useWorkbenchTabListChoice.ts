import { useMemoizedFn } from '@orchbee/foundation/react';
import type { TabStripSelectCallback } from '@orchbee/tab-strip';
import * as React from 'react';

/** The strip's touch long press, after which a release opens the Tab menu instead of choosing. */
const LONG_PRESS_MS = 500;

interface Press {
	readonly at: number;
	/** A drag or a long press: the release reorders or opens the Tab menu, never chooses. */
	cancelled: boolean;
	readonly primary: boolean;
	readonly touch: boolean;
}

export interface WorkbenchTabListChoice {
	readonly onDraggingChange: (dragging: boolean) => void;
	readonly onPointerCancelCapture: () => void;
	readonly onPointerDownCapture: (event: React.PointerEvent<HTMLElement>) => void;
	readonly onPointerUpCapture: (event: React.PointerEvent<HTMLElement>) => void;
	/** Absent without a host selection callback; the strip then selects on its own and stays open. */
	readonly onSelectTab: TabStripSelectCallback | undefined;
}

/**
 * Choosing a row closes the open Tab list once the selection settles and the press ends: a tap, a
 * click, Enter, or a Tab shortcut. A drag, a long press, a right click, or a modified click leaves
 * it open, so reordering and the Tab menu work as in the strip. While the list is closed its
 * strip still serves the Tab shortcuts, which then choose nothing.
 */
export function useWorkbenchTabListChoice({
	onChoose,
	open,
	selectTab,
}: {
	readonly onChoose: () => void;
	readonly open: boolean;
	readonly selectTab: TabStripSelectCallback | undefined;
}): WorkbenchTabListChoice {
	const press = React.useRef<Press | null>(null);
	const pending = React.useRef<{ settled: boolean } | null>(null);
	const finish = useMemoizedFn(() => {
		const choice = pending.current;
		if (choice === null || !choice.settled || press.current !== null) return;
		pending.current = null;
		onChoose();
	});
	const onSelectTab: TabStripSelectCallback = useMemoizedFn((context, input) => {
		const currentPress = press.current;
		const chosen =
			open &&
			input.mode === 'replace' &&
			(currentPress === null || (currentPress.primary && !currentPress.cancelled));
		const choice = chosen ? { settled: false } : null;
		if (chosen) pending.current = choice;
		const result = selectTab?.(context, input) ?? { kind: 'unchanged' as const };
		const settle = () => {
			if (choice === null || pending.current !== choice) return;
			choice.settled = true;
			finish();
		};
		void Promise.resolve(result).then(settle, settle);
		return result;
	});
	return {
		onDraggingChange: (dragging) => {
			if (!dragging) return;
			if (press.current !== null) press.current.cancelled = true;
			pending.current = null;
		},
		// A scroll that takes the touch chooses nothing.
		onPointerCancelCapture: () => {
			press.current = null;
			pending.current = null;
		},
		onPointerDownCapture: (event) => {
			press.current = {
				at: event.timeStamp,
				cancelled: false,
				primary: event.button === 0,
				touch: event.pointerType === 'touch',
			};
		},
		onPointerUpCapture: (event) => {
			const ended = press.current;
			if (ended === null) return;
			if (ended.cancelled || (ended.touch && event.timeStamp - ended.at >= LONG_PRESS_MS)) {
				ended.cancelled = true;
				pending.current = null;
			}
			// The strip chooses on release too; let its handlers run before the press ends.
			window.setTimeout(() => {
				if (press.current === ended) press.current = null;
				finish();
			}, 0);
		},
		onSelectTab: selectTab === undefined ? undefined : onSelectTab,
	};
}
