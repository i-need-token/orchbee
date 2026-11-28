import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import { codeEditorFontZoom } from '../../code-editor';
import type { FileOverlayEditorHost } from '../../file-overlay';
import { registerWorkbenchCodeEditorActions } from '../actions/registerWorkbenchCodeEditorActions';
import { WORKBENCH_ACTION_IDS } from '../actions/workbench-action-ids';
import type { WorkbenchShortcutPlatform } from '../types/workbench-shortcut-platform';
import { resolveWorkbenchFocusRegionFromTarget } from '../utils/workbench-focus-dom';
import {
	formatWorkbenchShortcut,
	type WorkbenchShortcutBindings,
} from '../utils/workbench-shortcuts';
import type { WorkbenchAuthorityNoticeView } from './useWorkbenchAuthorityNotice';

interface UseWorkbenchCodeEditorOptions {
	readonly actionRuntime: ActionRuntime;
	readonly actionScope: ActionScope;
	/** The keys configured commands hold, which run those commands in place of the Workbench's. */
	readonly heldLabels: ReadonlySet<string>;
	/** Whether the window offers the File Bar, whose overlay shows the editors. */
	readonly offered: boolean;
	readonly reportActionOutcome: WorkbenchAuthorityNoticeView['reportActionOutcome'];
	readonly shortcutBindings: WorkbenchShortcutBindings;
	readonly shortcutPlatform: WorkbenchShortcutPlatform;
}

/**
 * What the overlay's text editor needs of the Workbench, and the editor font size Actions, which
 * exist wherever the File Bar is offered.
 */
export function useWorkbenchCodeEditor({
	actionRuntime,
	actionScope,
	heldLabels,
	offered,
	reportActionOutcome,
	shortcutBindings,
	shortcutPlatform,
}: UseWorkbenchCodeEditorOptions): FileOverlayEditorHost {
	React.useEffect(() => {
		if (!offered) return undefined;
		const registration = registerWorkbenchCodeEditorActions({
			runtime: actionRuntime,
			scope: actionScope,
			zoom: codeEditorFontZoom,
		});
		return () => registration.dispose();
	}, [actionRuntime, actionScope, offered]);
	const searchActions = useMemoizedFn(() => {
		const invocation = { scope: actionScope, source: 'context-menu' } as const;
		reportActionOutcome(invocation.source, () =>
			actionRuntime.execute(WORKBENCH_ACTION_IDS.openSearch, { filter: 'actions' }, invocation),
		);
	});
	// VS Code's menu shows Command Palette's keys, which are Search actions' in Orchbee.
	const shortcut = formatWorkbenchShortcut('actionsSearch', shortcutPlatform, shortcutBindings);
	const searchActionsShortcut = heldLabels.has(shortcut) ? undefined : shortcut;
	return React.useMemo(
		() => ({ actionRuntime, actionScope, inFocusRegion, searchActions, searchActionsShortcut }),
		[actionRuntime, actionScope, searchActions, searchActionsShortcut],
	);
}

function inFocusRegion(target: EventTarget): boolean {
	return resolveWorkbenchFocusRegionFromTarget(target) !== null;
}
