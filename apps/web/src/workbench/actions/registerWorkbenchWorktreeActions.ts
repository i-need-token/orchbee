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
	WorkbenchWorktreeFormAnswer,
	WorkbenchWorktreeFormRequest,
} from '../types/workbench-worktree-form';
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
	/** Asks for the new Worktree's branch and base before anything is created. */
	readonly requestWorktreeForm: WorkbenchWorktreeFormRequest;
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
 * progress and outcomes; these Actions own the person's decisions. New Worktree asks for a branch and
 * base in the New Worktree form and creates nothing until it is submitted. Removal asks first; a
 * removal that stops reports its cause, and Remove Worktree… then asks again and continues it.
 */
export function registerWorkbenchWorktreeActions({
	projectWorkspaceRuntime,
	requestWorktreeForm,
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
	/**
	 * The TUI's naming rule: a blank branch is generated and the Workspace takes the first unused
	 * default name, while a typed branch names the Workspace through the Server.
	 */
	const creation = (answer: WorkbenchWorktreeFormAnswer) => ({
		...(answer.branch === null ? { label: defaultLabel() } : { branch: answer.branch }),
		...(answer.base === null ? {} : { base: answer.base }),
		kind: 'create' as const,
	});
	/** A removal that stops reports its cause; Remove Worktree… again continues it. */
	const removalResult = (outcome: WorkbenchWorktreeOutcome): ActionResult<void> =>
		outcome.kind === 'opened' || outcome.kind === 'removed'
			? { kind: 'completed' }
			: failedResult(outcome);

	return [
		typedRuntime.register(
			scope,
			defineAction({
				category,
				execution: workspaceExecution,
				getAvailability: (_context, input) => workspaceAvailability(input),
				handler: {
					execute: async (context, input) => {
						const answer = await requestWorktreeForm({ branch: '' }, context.signal);
						if (answer === null) return { kind: 'cancelled' };
						// The Workspace may have closed while the form was open.
						if (workspaceName(input.workspaceId) === undefined) {
							return { kind: 'blocked', code: 'workspace-unavailable' };
						}
						return openingResult(
							await send(context, { ...creation(answer), workspaceId: input.workspaceId }),
						);
					},
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
						// The form acts on the Panel that was focused when it opened, as the TUI's form does.
						const focused = resolveFocusedPanel(projectWorkspaceRuntime);
						if (focused === null) return { kind: 'blocked', code: 'focused-panel-unavailable' };
						const agentStart = input?.agentStart;
						// Start Agent proposes a branch for its Agent, which stays editable.
						const answer = await requestWorktreeForm(
							{ branch: agentStart === undefined ? '' : `agent/${agentStart}` },
							context.signal,
						);
						if (answer === null) return { kind: 'cancelled' };
						// Git's answer is unknown until the authority reads where the Panel stands.
						return openingResult(
							await send(context, {
								...creation(answer),
								...(agentStart === undefined ? {} : { agentStart }),
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
				// The removal's own question, with the cause it stopped at in place of the plan.
				confirmation: (input) => ({
					cancelLabel: workbenchMessages.panelCancel,
					confirmLabel: workbenchMessages.worktreeRemoveConfirm,
					// Remove holds the focus, so Enter removes, as the TUI's confirmation does.
					defaultAnswer: 'confirm',
					description: workbenchWorktreeFailureDescriptor('remove', input.cause),
					risk: 'destructive',
					title: workbenchMessages.worktreeRemoveTitle,
				}),
				execution: {
					// A removal that stops again is asked about again only from the menu.
					concurrency: 'serialize',
					concurrencyKey: (input) => input.operationId,
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: async (context, input) =>
						removalResult(await send(context, { kind: 'resume', operationId: input.operationId })),
				},
				id: WORKBENCH_ACTION_IDS.resumeWorktreeRemoval,
				input: workbenchWorktreeResumeInput,
				title: workbenchMessages.worktreeRemove,
			}),
		),
	];
}
