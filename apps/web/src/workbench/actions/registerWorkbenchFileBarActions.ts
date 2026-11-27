import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { fileBarMessages, workbenchMessages } from '@orchbee/i18n';

import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from './workbench-action-ids';

interface RegisterWorkbenchFileBarActionsOptions {
	/** Whether the File Bar shows now, which Toggle File Bar reports as its checked state. */
	readonly isVisible: boolean;
	readonly scope: ActionScope;
	readonly toggle: () => void;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

/**
 * The File Bar's Actions. They exist only in a window that offers the File Bar, so a release lists
 * none of them anywhere.
 */
export function registerWorkbenchFileBarActions({
	isVisible,
	scope,
	toggle,
	typedRuntime,
}: RegisterWorkbenchFileBarActionsOptions): ActionDisposable {
	return typedRuntime.register(
		scope,
		defineAction({
			category: {
				id: 'view',
				title: workbenchMessages.actionCategoryView,
			},
			execution: {
				concurrency: 'parallel',
				concurrencyKey: () => 'workbench',
				mode: 'foreground',
			},
			getAvailability: () => ({ checked: isVisible, kind: 'enabled' }),
			handler: {
				execute: (context) => {
					context.markCommitted();
					toggle();
					return { kind: 'completed' };
				},
			},
			// VS Code parity: workbench.action.toggleSidebarVisibility, on the File Bar.
			id: WORKBENCH_ACTION_IDS.toggleFileBar,
			input: noActionInput,
			title: fileBarMessages.toggle,
		}),
	);
}
