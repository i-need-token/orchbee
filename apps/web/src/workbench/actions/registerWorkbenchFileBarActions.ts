import {
	defineAction,
	noActionInput,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionResult,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { fileBarMessages, workbenchMessages } from '@orchbee/i18n';

import type { FileBarViewId } from '../../file-bar';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from './workbench-action-ids';

interface RegisterWorkbenchFileBarActionsOptions {
	/** Whether the File Bar shows now, which Toggle File Bar reports as its checked state. */
	readonly isVisible: boolean;
	readonly scope: ActionScope;
	/** Shows a view and focuses it, or returns focus to the terminal from that view when focused. */
	readonly showView: (view: FileBarViewId) => void;
	readonly toggle: () => void;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

const VIEW_CATEGORY = {
	id: 'view',
	title: workbenchMessages.actionCategoryView,
};

/**
 * The File Bar's Actions. They exist only in a window that offers the File Bar, so a release lists
 * none of them anywhere.
 */
export function registerWorkbenchFileBarActions({
	isVisible,
	scope,
	showView,
	toggle,
	typedRuntime,
}: RegisterWorkbenchFileBarActionsOptions): ActionDisposable {
	const execution = {
		concurrency: 'parallel',
		concurrencyKey: () => 'workbench',
		mode: 'foreground',
	} as const;
	const showViewHandler = (view: FileBarViewId) => ({
		execute: (context: ActionExecutionContext): ActionResult<void> => {
			context.markCommitted();
			showView(view);
			return { kind: 'completed' };
		},
	});
	const registrations = [
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution,
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
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution,
				getAvailability: () => ({ kind: 'enabled' }),
				handler: showViewHandler('explorer'),
				// VS Code parity: workbench.view.explorer.
				id: WORKBENCH_ACTION_IDS.showExplorer,
				input: noActionInput,
				title: fileBarMessages.showExplorer,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution,
				getAvailability: () => ({ kind: 'enabled' }),
				handler: showViewHandler('source-control'),
				// VS Code parity: workbench.view.scm.
				id: WORKBENCH_ACTION_IDS.showSourceControl,
				input: noActionInput,
				title: fileBarMessages.showSourceControl,
			}),
		),
	];
	return {
		dispose: () => {
			for (const registration of registrations) registration.dispose();
		},
	};
}
