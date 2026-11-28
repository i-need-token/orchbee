import {
	defineAction,
	noActionInput,
	type ActionAvailability,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionResult,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { fileBarMessages, workbenchMessages } from '@orchbee/i18n';

import type { FileBarPosition, FileBarViewId } from '../../file-bar';
import { WORKBENCH_ACTION_IDS, type WorkbenchActionCatalog } from './workbench-action-ids';

interface RegisterWorkbenchFileBarActionsOptions {
	/** Whether the File Bar shows now, which Toggle File Bar reports as its checked state. */
	readonly isVisible: boolean;
	/**
	 * Writes the File Bar's side into the user's settings and says how the write went; absent where
	 * the window has no settings to write, which offers no move.
	 */
	readonly move: ((position: FileBarPosition) => Promise<ActionResult<void>>) | undefined;
	/** The side the File Bar takes now, which decides the move each Action offers. */
	readonly position: FileBarPosition;
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
	move,
	position,
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
	// VS Code's moves write the setting only when they change it, so each offers itself only then.
	const moveHandler = (target: FileBarPosition) => ({
		execute: (
			context: ActionExecutionContext,
		): Promise<ActionResult<void>> | ActionResult<void> => {
			if (move === undefined) return { kind: 'unsupported' };
			context.markCommitted();
			return move(target);
		},
	});
	const moveAvailability = (target: FileBarPosition) => (): ActionAvailability =>
		move === undefined || position === target ? { kind: 'hidden' } : { kind: 'enabled' };
	const otherSide: FileBarPosition = position === 'left' ? 'right' : 'left';
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
				getAvailability: () => (move === undefined ? { kind: 'hidden' } : { kind: 'enabled' }),
				handler: moveHandler(otherSide),
				// VS Code parity: workbench.action.toggleSidebarPosition, on the File Bar.
				id: WORKBENCH_ACTION_IDS.toggleFileBarPosition,
				input: noActionInput,
				title: fileBarMessages.togglePosition,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution,
				getAvailability: moveAvailability('left'),
				handler: moveHandler('left'),
				// VS Code parity: workbench.action.moveSideBarLeft, on the File Bar.
				id: WORKBENCH_ACTION_IDS.moveFileBarLeft,
				input: noActionInput,
				title: fileBarMessages.moveLeft,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category: VIEW_CATEGORY,
				execution,
				getAvailability: moveAvailability('right'),
				handler: moveHandler('right'),
				// VS Code parity: workbench.action.moveSideBarRight, on the File Bar.
				id: WORKBENCH_ACTION_IDS.moveFileBarRight,
				input: noActionInput,
				title: fileBarMessages.moveRight,
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
