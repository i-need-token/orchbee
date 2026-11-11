import {
	defineAction,
	type ActionDisposable,
	type ActionExecutionContext,
	type ActionResult,
	type ActionScope,
	type TypedActionRuntime,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import { workbenchWorktreeFailureDescriptor } from '../model/workbench-worktree-failure';
import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchWorktreeCommand,
	WorkbenchWorktreeOutcome,
	WorkbenchWorktrees,
} from '../types/workbench-worktrees';
import { resolveFocusedPanel } from '../utils/resolveFocusedPanel';
import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	type WorkbenchWorktreeOpening,
	workbenchAgentStartOption,
	workbenchFocusedPanelWorktreeCheckoutInput,
	workbenchWorkspaceInput,
	workbenchWorktreeCheckoutInput,
	workbenchWorktreeRemovalInput,
	workbenchWorktreeResumeInput,
} from './workbench-action-ids';

interface RegisterWorkbenchWorktreeActionsOptions {
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly scope: ActionScope;
	readonly typedRuntime: TypedActionRuntime<WorkbenchActionCatalog>;
	/** The localized default Workspace label with this ordinal. */
	readonly worktreeLabel: (ordinal: number) => string;
	readonly worktrees: WorkbenchWorktrees;
}

type WorktreeFailure = Extract<WorkbenchWorktreeOutcome, { readonly kind: 'failed' | 'unknown' }>;

function failedResult(outcome: WorktreeFailure) {
	return {
		failure: { code: outcome.kind === 'failed' ? outcome.cause : 'outcome-unknown' },
		kind: 'failed' as const,
	};
}

function openingResult(outcome: WorkbenchWorktreeOutcome): ActionResult<WorkbenchWorktreeOpening> {
	switch (outcome.kind) {
		case 'opened':
			return { kind: 'completed', value: { workspaceId: outcome.workspaceId } };
		case 'removed':
			return { kind: 'completed' };
		case 'failed':
		case 'unknown':
			return failedResult(outcome);
	}
}

/**
 * The Workspace menu's Worktree commands, and the TUI's `Ctrl+B W` and `Ctrl+B O` as Search's
 * commands from the focused Panel. The authority owns Git and the operation, and its notices report
 * progress and outcomes; these Actions own the person's decisions. Removal asks first, and a removal
 * that stops where it may continue asks again before it continues.
 */
export function registerWorkbenchWorktreeActions({
	projectWorkspaceRuntime,
	scope,
	typedRuntime,
	worktreeLabel,
	worktrees,
}: RegisterWorkbenchWorktreeActionsOptions): readonly ActionDisposable[] {
	const category = { id: 'projects', title: workbenchMessages.actionCategoryProjects };
	const workspaceExecution = {
		concurrency: 'reject-duplicate' as const,
		concurrencyKey: (input: { readonly workspaceId: string }) => input.workspaceId,
		mode: 'foreground' as const,
	};
	const workspaceName = (workspaceId: string) =>
		projectWorkspaceRuntime
			.getSnapshot()
			.workspaces.find((workspace) => workspace.id === workspaceId)?.name;
	const workspaceAvailability = (input: { readonly workspaceId: string }) =>
		workspaceName(input.workspaceId) === undefined
			? { kind: 'disabled' as const, reason: workbenchMessages.workspaceUnavailableMissing }
			: { kind: 'enabled' as const };
	/** Search offers the focused Panel's commands only while a Panel is focused. */
	const focusedPanelAvailability = () =>
		resolveFocusedPanel(projectWorkspaceRuntime) === null
			? {
					kind: 'disabled' as const,
					reason: workbenchMessages.panelUnavailableFocusedPanelRequired,
				}
			: { kind: 'enabled' as const };
	const send = (context: ActionExecutionContext, command: WorkbenchWorktreeCommand) => {
		// The Server owns an operation from the moment it is asked to start one.
		context.markCommitted();
		return worktrees.run(command);
	};
	/** The TUI's default: the first ordinal no Workspace is named after. */
	const defaultLabel = () => {
		const names = new Set(
			projectWorkspaceRuntime.getSnapshot().workspaces.map((workspace) => workspace.name),
		);
		let ordinal = 1;
		while (names.has(worktreeLabel(ordinal))) ordinal += 1;
		return worktreeLabel(ordinal);
	};
	const removalResult = (
		context: ActionExecutionContext,
		outcome: WorkbenchWorktreeOutcome,
	): ActionResult<void> => {
		if (outcome.kind === 'opened' || outcome.kind === 'removed') return { kind: 'completed' };
		if (outcome.kind === 'failed' && outcome.resumable !== null) {
			// Continuing is its own question, asked once this removal has stopped.
			typedRuntime.execute(
				WORKBENCH_ACTION_IDS.resumeWorktreeRemoval,
				{ cause: outcome.resumable.cause, operationId: outcome.resumable.operationId },
				{ scope, source: context.source },
			);
		}
		return failedResult(outcome);
	};

	return [
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: workspaceExecution,
				getAvailability: (_context, input) => workspaceAvailability(input),
				handler: {
					execute: async (context, input) =>
						openingResult(
							await send(context, {
								kind: 'create',
								label: defaultLabel(),
								workspaceId: input.workspaceId,
							}),
						),
				},
				id: WORKBENCH_ACTION_IDS.createWorktree,
				input: workbenchWorkspaceInput,
				title: workbenchMessages.worktreeNew,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: workspaceExecution,
				getAvailability: (_context, input) => workspaceAvailability(input),
				handler: {
					execute: async (context, input) =>
						openingResult(
							await send(context, {
								kind: 'open',
								path: input.path,
								workspaceId: input.workspaceId,
							}),
						),
				},
				id: WORKBENCH_ACTION_IDS.openWorktree,
				input: workbenchWorktreeCheckoutInput,
				title: workbenchMessages.worktreeOpen,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: {
					concurrency: 'reject-duplicate',
					concurrencyKey: () => 'focused-panel',
					mode: 'foreground',
				},
				getAvailability: focusedPanelAvailability,
				handler: {
					execute: async (context, input) => {
						const focused = resolveFocusedPanel(projectWorkspaceRuntime);
						if (focused === null) return { kind: 'blocked', code: 'focused-panel-unavailable' };
						// Git's answer is unknown until the authority reads where the Panel stands.
						return openingResult(
							await send(context, {
								...(input?.agentStart === undefined ? {} : { agentStart: input.agentStart }),
								kind: 'create',
								label: defaultLabel(),
								panel: { panelId: focused.panelId, presentationId: focused.presentationId },
								workspaceId: focused.workspaceId,
							}),
						);
					},
				},
				id: WORKBENCH_ACTION_IDS.createFocusedPanelWorktree,
				input: workbenchAgentStartOption,
				title: workbenchMessages.worktreeNew,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: workspaceExecution,
				getAvailability: (_context, input) => workspaceAvailability(input),
				// Search picks the checkout from what Git listed where the focused Panel stands.
				getDiscoveryAvailability: focusedPanelAvailability,
				handler: {
					execute: async (context, input) =>
						openingResult(
							await send(context, {
								kind: 'open',
								panel: { panelId: input.panelId, presentationId: input.presentationId },
								path: input.path,
								workspaceId: input.workspaceId,
							}),
						),
				},
				id: WORKBENCH_ACTION_IDS.openFocusedPanelWorktree,
				input: workbenchFocusedPanelWorktreeCheckoutInput,
				title: workbenchMessages.worktreeOpen,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				confirmation: (input) => ({
					cancelLabel: workbenchMessages.panelCancel,
					confirmLabel: workbenchMessages.worktreeRemoveConfirm,
					// Remove holds the focus, so Enter removes, as the TUI's confirmation does.
					defaultAnswer: 'confirm',
					description: {
						...workbenchMessages.worktreeRemoveDescription,
						values: {
							branch: input.branch ?? '',
							kept: input.branch !== null,
							path: input.path,
							terminals: input.terminalCount,
							workspace: workspaceName(input.workspaceId) ?? '',
						},
					},
					risk: 'destructive',
					title: workbenchMessages.worktreeRemoveTitle,
				}),
				execution: workspaceExecution,
				getAvailability: (_context, input) => workspaceAvailability(input),
				handler: {
					execute: async (context, input) =>
						removalResult(
							context,
							await send(context, {
								kind: 'remove',
								path: input.path,
								workspaceId: input.workspaceId,
							}),
						),
				},
				id: WORKBENCH_ACTION_IDS.removeWorktree,
				input: workbenchWorktreeRemovalInput,
				title: workbenchMessages.worktreeRemove,
			}),
		),
		typedRuntime.register(
			scope,
			defineAction({
				category,
				confirmation: (input) => ({
					cancelLabel: workbenchMessages.panelCancel,
					confirmLabel: workbenchMessages.worktreeResume,
					// Resume Removal holds the focus, so Enter resumes, as the TUI's stopped removal does.
					defaultAnswer: 'confirm',
					description: workbenchWorktreeFailureDescriptor('remove', input.cause),
					risk: 'destructive',
					title: workbenchMessages.worktreeStoppedTitle,
				}),
				execution: {
					// A Resume that stops again asks again, once its own execution has finished.
					concurrency: 'serialize',
					concurrencyKey: (input) => input.operationId,
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: async (context, input) =>
						removalResult(
							context,
							await send(context, { kind: 'resume', operationId: input.operationId }),
						),
				},
				id: WORKBENCH_ACTION_IDS.resumeWorktreeRemoval,
				input: workbenchWorktreeResumeInput,
				title: workbenchMessages.worktreeResume,
			}),
		),
	];
}
