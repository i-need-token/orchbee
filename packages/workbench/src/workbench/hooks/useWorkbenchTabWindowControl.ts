import type { ActionResult } from '@orchbee/actions';
import type {
	TabModel,
	TabStripCrossWindowDragRuntimeFactory,
	TabStripMoveGroupToWindowCallback,
	TabStripMoveTabsToWindowCallback,
	TabStripWindowTarget,
} from '@orchbee/tab-strip';
import * as React from 'react';

import { WorkbenchTabWindowDragRuntime } from '../model/WorkbenchTabWindowDragRuntime';
import type {
	WorkbenchTabWindowControl,
	WorkbenchTabWindowDestination,
	WorkbenchTabWindowMoveResult,
	WorkbenchTabWindowSnapshot,
} from '../types/workbench-tab-window-control';

const EMPTY_WINDOW_SNAPSHOT: WorkbenchTabWindowSnapshot = Object.freeze({
	currentWindowId: '',
	revision: '0',
	terminalContexts: Object.freeze([]),
	windows: Object.freeze([]),
});

interface UseWorkbenchTabWindowControlOptions {
	readonly control: WorkbenchTabWindowControl | undefined;
	readonly formatWindowLabel: (ordinal: number) => string;
	readonly model: TabModel;
	readonly terminalContextId: string;
}

interface WorkbenchTabWindowIntegration {
	readonly createCrossWindowDragRuntime: TabStripCrossWindowDragRuntimeFactory | undefined;
	readonly moveGroupToWindow: TabStripMoveGroupToWindowCallback | undefined;
	readonly moveTabsToWindow: TabStripMoveTabsToWindowCallback | undefined;
	readonly windowTargets: readonly TabStripWindowTarget[];
}

export function useWorkbenchTabWindowControl({
	control,
	formatWindowLabel,
	model,
	terminalContextId,
}: UseWorkbenchTabWindowControlOptions): WorkbenchTabWindowIntegration {
	const getNoWindows = (): WorkbenchTabWindowSnapshot => EMPTY_WINDOW_SNAPSHOT;
	const subscribeToNoWindows = (): (() => void) => () => undefined;
	const destination = (destinationWindowId: string | undefined): WorkbenchTabWindowDestination =>
		destinationWindowId === undefined
			? { kind: 'new' }
			: { kind: 'existing', windowId: destinationWindowId };
	const actionResultFromWindowMove = (
		result: WorkbenchTabWindowMoveResult,
		markCommitted: () => void,
	): ActionResult<void> => {
		if (result.kind === 'completed') {
			markCommitted();
			return { kind: 'completed' };
		}
		if (result.kind === 'unchanged') return result;
		return { failure: { code: result.code }, kind: 'failed' };
	};
	const snapshot = React.useSyncExternalStore(
		control?.subscribe ?? subscribeToNoWindows,
		control?.getSnapshot ?? getNoWindows,
		control?.getSnapshot ?? getNoWindows,
	);
	const windowTargets = React.useMemo(
		() =>
			snapshot.windows
				.filter((window) => window.windowId !== snapshot.currentWindowId)
				.map((window) => ({
					label: formatWindowLabel(window.ordinal),
					windowId: window.windowId,
				})),
		[formatWindowLabel, snapshot],
	);
	const moveTabsToWindow = React.useMemo<TabStripMoveTabsToWindowCallback | undefined>(
		() =>
			control === undefined
				? undefined
				: async (context, input) =>
						actionResultFromWindowMove(
							await control.moveTabs(
								terminalContextId,
								input.tabIds,
								destination(input.destinationWindowId),
							),
							() => context.markCommitted(),
						),
		[control, terminalContextId],
	);
	const moveGroupToWindow = React.useMemo<TabStripMoveGroupToWindowCallback | undefined>(
		() =>
			control === undefined
				? undefined
				: async (context, input) =>
						actionResultFromWindowMove(
							await control.moveGroup(
								terminalContextId,
								input.groupId,
								destination(input.destinationWindowId),
							),
							() => context.markCommitted(),
						),
		[control, terminalContextId],
	);
	const createCrossWindowDragRuntime = React.useMemo<
		TabStripCrossWindowDragRuntimeFactory | undefined
	>(
		() =>
			control === undefined
				? undefined
				: (sessionData) =>
						new WorkbenchTabWindowDragRuntime({
							control,
							model,
							sessionData,
							terminalContextId,
						}),
		[control, model, terminalContextId],
	);

	return { createCrossWindowDragRuntime, moveGroupToWindow, moveTabsToWindow, windowTargets };
}
