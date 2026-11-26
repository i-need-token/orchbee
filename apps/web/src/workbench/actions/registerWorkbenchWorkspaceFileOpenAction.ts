import {
	defineAction,
	type ActionDisposable,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchWorkspaceFileOpenInput,
	workbenchWorkspaceFileOpenInput,
} from './workbench-action-ids';

interface RegisterWorkbenchWorkspaceFileOpenActionOptions {
	/** The Workspace whose files show; a file of another one is not opened. */
	readonly activeWorkspaceId: string | null;
	readonly open: (input: WorkbenchWorkspaceFileOpenInput) => void;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
}

/**
 * Open File: opens a file of the shown Workspace in its float window as a kept tab, at a line and
 * column when given. Quick open's rows run it; it has no key and lists nowhere without a file.
 */
export function registerWorkbenchWorkspaceFileOpenAction({
	activeWorkspaceId,
	open,
	scope,
	typedRuntime,
}: RegisterWorkbenchWorkspaceFileOpenActionOptions): ActionDisposable {
	return typedRuntime.register(
		scope,
		defineAction({
			category: { id: 'navigation', title: workbenchMessages.actionCategoryNavigation },
			execution: {
				concurrency: 'replace-latest',
				concurrencyKey: () => 'workspace-file-open',
				mode: 'foreground',
			},
			getAvailability: (_context, input) =>
				input.workspaceId === activeWorkspaceId ? { kind: 'enabled' } : { kind: 'disabled' },
			handler: {
				execute: (context, input) => {
					if (context.signal.aborted) return { kind: 'cancelled' };
					if (input.workspaceId !== activeWorkspaceId) {
						return { code: 'workspace-file-workspace-unavailable', kind: 'blocked' };
					}
					context.markCommitted();
					open(input);
					return { kind: 'completed' };
				},
			},
			id: WORKBENCH_ACTION_IDS.openWorkspaceFile,
			input: workbenchWorkspaceFileOpenInput,
			title: workbenchMessages.openWorkspaceFile,
		}),
	);
}
