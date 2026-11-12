import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';

import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchWorktreeFormAnswer,
	WorkbenchWorktreeFormRequest,
} from '../types/workbench-worktree-form';
import { findActiveTerminalFocusTarget } from '../utils/findActiveTerminalFocusTarget';

/** What the New Worktree form shows while it is open. */
export interface WorkbenchWorktreeFormModel {
	/** The Branch field's starting text. */
	readonly branch: string;
	readonly finalFocus: () => HTMLElement | null;
	/** Changes with each request, so a new form starts from its own prefill. */
	readonly key: number;
	/** Cancel, Escape, the close button and the backdrop: nothing is created. */
	readonly onCancel: () => void;
	readonly onSubmit: (answer: WorkbenchWorktreeFormAnswer) => void;
}

interface Pending {
	readonly id: number;
	readonly settle: (answer: WorkbenchWorktreeFormAnswer | null) => void;
}

interface UseWorkbenchWorktreeFormOptions {
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
}

/**
 * The one owner of the New Worktree form. Each request opens it and settles once: with what it
 * submitted, or with null on cancel, on the requester's abort, on a newer request, or on unmount.
 * Closing returns the keyboard to where it was when the form opened, or to the active terminal when
 * that place is gone, as when Start Agent closed to open it.
 */
export function useWorkbenchWorktreeForm({
	projectWorkspaceRuntime,
}: UseWorkbenchWorktreeFormOptions): {
	readonly form: WorkbenchWorktreeFormModel | undefined;
	readonly request: WorkbenchWorktreeFormRequest;
} {
	const [shown, setShown] = React.useState<{ readonly branch: string; readonly id: number } | null>(
		null,
	);
	const pending = React.useRef<Pending | null>(null);
	const sequence = React.useRef(0);
	const returnFocus = React.useRef<HTMLElement | null>(null);

	const finish = useMemoizedFn((id: number, answer: WorkbenchWorktreeFormAnswer | null) => {
		const current = pending.current;
		if (current?.id !== id) return;
		pending.current = null;
		setShown(null);
		current.settle(answer);
	});
	React.useEffect(
		() => () => {
			const current = pending.current;
			pending.current = null;
			current?.settle(null);
		},
		[],
	);

	const request = useMemoizedFn<
		void,
		Parameters<WorkbenchWorktreeFormRequest>,
		ReturnType<WorkbenchWorktreeFormRequest>
	>(
		(prefill, signal) =>
			new Promise((resolve) => {
				const previous = pending.current;
				if (previous !== null) finish(previous.id, null);
				if (signal.aborted) {
					resolve(null);
					return;
				}
				const id = ++sequence.current;
				const abort = () => finish(id, null);
				signal.addEventListener('abort', abort, { once: true });
				const focused = typeof document === 'undefined' ? null : document.activeElement;
				returnFocus.current =
					focused instanceof HTMLElement && focused !== document.body ? focused : null;
				pending.current = {
					id,
					settle: (answer) => {
						signal.removeEventListener('abort', abort);
						resolve(answer);
					},
				};
				setShown({ branch: prefill.branch, id });
			}),
	);
	const finalFocus = useMemoizedFn((): HTMLElement | null => {
		const captured = returnFocus.current;
		returnFocus.current = null;
		return captured?.isConnected === true
			? captured
			: findActiveTerminalFocusTarget(projectWorkspaceRuntime);
	});

	return {
		form:
			shown === null
				? undefined
				: {
						branch: shown.branch,
						finalFocus,
						key: shown.id,
						onCancel: () => finish(shown.id, null),
						onSubmit: (answer) => finish(shown.id, answer),
					},
		request,
	};
}
