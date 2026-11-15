import type { ActionResult } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import type { TabStripSelectCallback } from '@orchbee/tab-strip';
import * as React from 'react';

import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type { WorkbenchTerminalHandover } from '../types/workbench-focus-routing';

interface PendingTabFocus {
	readonly handover: WorkbenchTerminalHandover;
	readonly originTabId: string | null;
	readonly tabId: string;
	readonly terminalContextId: string;
}

interface UseWorkbenchTabSelectionFocusOptions {
	readonly beginTerminalHandover: () => WorkbenchTerminalHandover;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly selectTab: TabStripSelectCallback | undefined;
}

/**
 * Switching to a Tab, by any entry path, gives the keyboard to the Panel that Tab focused last, as
 * the TUI's switch lands on the Tab's remembered Panel. Each Tab keeps that Panel in its view, so
 * the terminal handover only waits until the Tab is shown, and until a pointer press ends, because
 * the pressed Tab takes focus as the press goes down. A press or key since the switch began keeps
 * focus where the person put it.
 */
export function useWorkbenchTabSelectionFocus({
	beginTerminalHandover,
	projectWorkspaceRuntime,
	selectTab,
}: UseWorkbenchTabSelectionFocusOptions): TabStripSelectCallback | undefined {
	const pending = React.useRef<PendingTabFocus | null>(null);
	const pressedPointers = React.useRef(new Set<number>());

	const settle = useMemoizedFn((): void => {
		const current = pending.current;
		if (current === null || pressedPointers.current.size > 0) return;
		const context = projectWorkspaceRuntime.getActiveTerminalContext();
		const activeTabId = context?.tabModel.getSnapshot().activeTabId ?? null;
		const shown = context?.terminalContextId === current.terminalContextId;
		if (shown && activeTabId === current.originTabId) return;
		pending.current = null;
		// Another Tab or Workspace took the switch's place, so focus stays.
		if (!shown || activeTabId !== current.tabId) return;
		current.handover.complete();
	});

	React.useEffect(
		() => projectWorkspaceRuntime.subscribeNavigation(settle),
		[projectWorkspaceRuntime, settle],
	);
	React.useEffect(() => {
		const pressed = pressedPointers.current;
		const press = (event: PointerEvent): void => {
			pressed.add(event.pointerId);
		};
		const release = (event: PointerEvent): void => {
			if (!pressed.delete(event.pointerId)) return;
			settle();
		};
		window.addEventListener('pointerdown', press, true);
		window.addEventListener('pointerup', release, true);
		window.addEventListener('pointercancel', release, true);
		return () => {
			window.removeEventListener('pointerdown', press, true);
			window.removeEventListener('pointerup', release, true);
			window.removeEventListener('pointercancel', release, true);
			pressed.clear();
		};
	}, [settle]);

	const select = useMemoizedFn<
		void,
		Parameters<TabStripSelectCallback>,
		ReturnType<TabStripSelectCallback>
	>((context, input) => {
		if (selectTab === undefined) throw new Error('Tab selection is unavailable.');
		const origin = projectWorkspaceRuntime.getActiveTerminalContext();
		// Only a plain choice switches; adding Tabs to a selection leaves focus where it is.
		if (input.mode !== 'replace' || origin === null) return selectTab(context, input);
		const handover = beginTerminalHandover();
		const originTabId = origin.tabModel.getSnapshot().activeTabId;
		const follow = (result: ActionResult<void>): ActionResult<void> => {
			if (result.kind !== 'completed') return result;
			pending.current = {
				handover,
				originTabId,
				tabId: input.tabId,
				terminalContextId: origin.terminalContextId,
			};
			settle();
			return result;
		};
		const result = selectTab(context, input);
		return result instanceof Promise ? result.then(follow) : follow(result);
	});

	return selectTab === undefined ? undefined : select;
}
