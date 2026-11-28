import type {
	ActionExecution,
	ActionInvocationSource,
	ActionScope,
	TypedActionRuntime,
} from '@orchbee/actions';
import { fileBarMessages, type MessageDescriptor } from '@orchbee/i18n';

import {
	FILE_BAR_VIEW_IDS,
	type FileBarLayout,
	type FileBarPosition,
	type FileBarRowMenuEntry,
	type FileBarViewId,
} from '../../file-bar';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from '../actions/workbench-action-ids';

/** Each view's checkbox, labelled with its name as VS Code labels a container's. */
const VIEW_LABELS: Readonly<Record<FileBarViewId, MessageDescriptor>> = {
	explorer: fileBarMessages.explorer,
	'source-control': fileBarMessages.sourceControl,
};

/** The Action that shows each view, whose shortcut its checkbox shows as VS Code's does. */
export function showFileBarViewActionId(
	view: FileBarViewId,
): typeof WORKBENCH_ACTION_IDS.showExplorer | typeof WORKBENCH_ACTION_IDS.showSourceControl {
	return view === 'explorer'
		? WORKBENCH_ACTION_IDS.showExplorer
		: WORKBENCH_ACTION_IDS.showSourceControl;
}

interface FileBarRowMenuOptions {
	readonly formatMessage: (descriptor: MessageDescriptor) => string;
	/** The layout as the menu opens, whose hidden toggles the checkboxes show. */
	readonly layout: FileBarLayout;
	/** The File Bar's side now, which names the move the menu offers. */
	readonly position: FileBarPosition;
	/** Runs an item's Action and says a refusal or failure that no surface showed. */
	readonly report: (source: ActionInvocationSource, start: () => ActionExecution<unknown>) => void;
	/** The Workbench's own scope, where the File Bar's Actions resolve. */
	readonly scope: ActionScope;
	/** Shows or hides a view's toggle, as VS Code's checkboxes pin and unpin a container. */
	readonly setToggleShown: (view: FileBarViewId, shown: boolean) => void;
	readonly shortcutLabelForAction: (actionId: string) => string | undefined;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

/** An item that runs a File Bar Action, while the Action is offered. */
function commandOf(
	actionId:
		| typeof WORKBENCH_ACTION_IDS.toggleFileBar
		| typeof WORKBENCH_ACTION_IDS.toggleFileBarPosition,
	label: MessageDescriptor,
	{ formatMessage, report, scope, shortcutLabelForAction, typedRuntime }: FileBarRowMenuOptions,
): FileBarRowMenuEntry | null {
	const invocation = { scope, source: 'context-menu' } as const;
	const resolved = typedRuntime.resolveAction(actionId, undefined, invocation);
	if (resolved === null || resolved.availability.kind === 'hidden') return null;
	return {
		disabled: resolved.availability.kind === 'disabled',
		id: actionId,
		kind: 'command',
		label: formatMessage(label),
		run: () => report('context-menu', () => typedRuntime.execute(actionId, undefined, invocation)),
		shortcut: shortcutLabelForAction(actionId),
	};
}

/**
 * The toggles row's menu as it opens, VS Code's activity bar menu with File Bar wording: a checkbox
 * for each view, checked while its toggle shows and greyed for the last one shown; then Move File
 * Bar Left or Right, which toggles the side as VS Code's does, and Hide File Bar. Each item shows its
 * Action's shortcut, as VS Code's menu shows each action's keybinding.
 */
export function buildFileBarRowMenu(
	options: FileBarRowMenuOptions,
): readonly FileBarRowMenuEntry[] {
	const { formatMessage, layout, position, setToggleShown, shortcutLabelForAction } = options;
	const shownCount = FILE_BAR_VIEW_IDS.filter(
		(view) => !layout.hiddenToggles.includes(view),
	).length;
	const toggles = FILE_BAR_VIEW_IDS.map((view): FileBarRowMenuEntry => {
		const checked = !layout.hiddenToggles.includes(view);
		return {
			checked,
			disabled: checked && shownCount === 1,
			id: view,
			kind: 'toggle',
			label: formatMessage(VIEW_LABELS[view]),
			run: () => setToggleShown(view, !checked),
			shortcut: shortcutLabelForAction(showFileBarViewActionId(view)),
		};
	});
	const commands = [
		commandOf(
			WORKBENCH_ACTION_IDS.toggleFileBarPosition,
			position === 'right' ? fileBarMessages.moveLeft : fileBarMessages.moveRight,
			options,
		),
		commandOf(WORKBENCH_ACTION_IDS.toggleFileBar, fileBarMessages.hideFileBar, options),
	].filter((entry) => entry !== null);
	return commands.length === 0
		? toggles
		: [...toggles, { id: 'file-bar-place', kind: 'separator' }, ...commands];
}
