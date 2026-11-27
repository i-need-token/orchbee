import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { fileOverlayMessages, workbenchMessages } from '@orchbee/i18n';

import type { FileOverlayState } from '../../file-overlay';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from './workbench-action-ids';

interface RegisterWorkbenchFileOverlayActionsOptions {
	/** The shown Workspace's overlay mode now, read whenever an Action's availability is asked. */
	readonly getMode: () => FileOverlayState['mode'];
	readonly scope: ActionScope;
	/** Expands the open overlay to the whole window, or restores its tier from before. */
	readonly toggleFull: () => void;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

const VIEW_CATEGORY = {
	id: 'view',
	title: workbenchMessages.actionCategoryView,
};

/**
 * The file overlay's Actions. They exist only in a window that offers the File Bar, and each is
 * available only while the shown Workspace's overlay is open.
 */
export function registerWorkbenchFileOverlayActions({
	getMode,
	scope,
	toggleFull,
	typedRuntime,
}: RegisterWorkbenchFileOverlayActionsOptions): ActionDisposable {
	return typedRuntime.register(
		scope,
		defineAction({
			category: VIEW_CATEGORY,
			execution: {
				concurrency: 'parallel',
				concurrencyKey: () => 'workbench',
				mode: 'foreground',
			},
			getAvailability: () => {
				const mode = getMode();
				return mode.kind === 'open'
					? { checked: mode.tier === 'full', kind: 'enabled' }
					: { kind: 'hidden' };
			},
			handler: {
				execute: (context) => {
					context.markCommitted();
					toggleFull();
					return { kind: 'completed' };
				},
			},
			// VS Code parity: workbench.action.toggleMaximizeEditorGroup.
			id: WORKBENCH_ACTION_IDS.toggleFileOverlayFull,
			input: noActionInput,
			title: fileOverlayMessages.toggleFull,
		}),
	);
}
