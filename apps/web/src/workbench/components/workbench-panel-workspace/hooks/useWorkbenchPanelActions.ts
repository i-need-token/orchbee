import {
	createTypedActionRuntime,
	defineAction,
	type ActionCatalogId,
	type ActionExecution,
	type ActionInputFor,
	type ActionInvocationSource,
	type ActionMessageDescriptor,
	type ActionOutputFor,
	type ActionRuntime,
	type ActionScope,
} from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import { workbenchMessages } from '@orchbee/i18n';
import * as React from 'react';
import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import type { WorkbenchSharedPanelTerminalCreate } from '../../../actions/createSharedPanelTerminal';
import {
	terminalCloseImpact,
	terminalCloseWarningValues,
} from '../../../model/terminal-close-impact';
import type {
	WorkbenchAuthority,
	WorkbenchAuthorityClosure,
} from '../../../types/workbench-authority';
import {
	PANEL_ACTION_IDS,
	panelChildTerminalInput,
	panelCloseInput,
	panelMoveInput,
	panelResizeInput,
	panelSplitInput,
	panelSwapInput,
	panelTargetInput,
	panelVisualSelectionInput,
	type PanelActionCatalog,
	type PanelCloseInput,
	type PanelVisualSelectionInput,
} from '../actions/panel-action-contracts';
import {
	findPanel,
	getDisplayedTerminal,
	type PanelWorkspaceTransition,
	type TabPanelWorkspace,
	type TerminalPresentation,
} from '../model/panel-workspace-state';

interface PanelActionOperations {
	readonly activateChildTerminal: (
		panelId: string,
		childPresentationId: TerminalPresentationId,
	) => PanelWorkspaceTransition | null;
	readonly focusPanel: (panelId: string) => PanelWorkspaceTransition | null;
	readonly showPrimaryTerminal: (panelId: string) => PanelWorkspaceTransition | null;
	readonly startPanelResize: (panelId: string) => boolean;
	readonly startPanelVisualSelection: (
		panelId: string,
		mode: PanelVisualSelectionInput['mode'],
	) => boolean;
	readonly togglePanelMaximization: (panelId: string) => PanelWorkspaceTransition | null;
}

interface UseWorkbenchPanelActionsOptions {
	readonly activeTab: TabPanelWorkspace | null;
	/** Decides the shared Workbench structure. */
	readonly authority: WorkbenchAuthority;
	readonly createPanelTerminal: WorkbenchSharedPanelTerminalCreate;
	readonly operations: PanelActionOperations;
	readonly panelCount: number;
	readonly parentScope: ActionScope;
	readonly runtime: ActionRuntime;
	readonly terminalContextId: string;
}

interface WorkbenchPanelActions {
	readonly execute: <Id extends ActionCatalogId<PanelActionCatalog>>(
		actionId: Id,
		input: ActionInputFor<PanelActionCatalog, Id>,
		source: ActionInvocationSource,
	) => ActionExecution<ActionOutputFor<PanelActionCatalog, Id>>;
	readonly scope: ActionScope;
}

export function useWorkbenchPanelActions({
	activeTab,
	authority,
	createPanelTerminal,
	operations,
	panelCount,
	parentScope,
	runtime,
	terminalContextId,
}: UseWorkbenchPanelActionsOptions): WorkbenchPanelActions {
	const scope = React.useMemo(
		() => runtime.createScope('panel-workspace', parentScope),
		[parentScope, runtime],
	);
	const typedRuntime = React.useMemo(
		() => createTypedActionRuntime<PanelActionCatalog>(runtime),
		[runtime],
	);

	React.useEffect(() => {
		const category = {
			id: 'panels',
			title: workbenchMessages.actionCategoryPanels,
		};
		const execution = {
			concurrency: 'parallel' as const,
			concurrencyKey: (input: { readonly panelId: string }) => input.panelId,
			mode: 'foreground' as const,
		};
		const findTarget = (panelId: string) =>
			activeTab === null ? null : findPanel(activeTab.root, panelId);
		const findCloseTarget = (input: PanelCloseInput) =>
			activeTab?.tabId === input.tabId ? findTarget(input.panelId) : null;
		/** As in the TUI, a Panel's Close ends every terminal it holds, whichever one it shows. */
		const closedTerminals = (input: PanelCloseInput): readonly TerminalPresentation[] => {
			const panel = findCloseTarget(input);
			return panel === null ? [] : [panel.primaryPresentation, ...panel.childPresentations];
		};
		const closureFor = (
			input: PanelCloseInput,
			type: 'panel-close' | 'panel-close-if-idle',
		): WorkbenchAuthorityClosure => ({
			panelId: input.panelId,
			tabId: input.tabId,
			terminalContextId,
			type,
		});
		/**
		 * As in the TUI, several Panels close side by side: a close waits only for an earlier close of
		 * its own Panel, so another Panel asks or closes at once while an earlier close is followed.
		 */
		const closeExecution = {
			concurrency: 'serialize' as const,
			concurrencyKey: (input: PanelCloseInput) =>
				`${terminalContextId}:${input.tabId}:${input.panelId}`,
			mode: 'foreground' as const,
		};
		const disabled = (reason: ActionMessageDescriptor) => ({ kind: 'disabled' as const, reason });
		const panelMissing = disabled(workbenchMessages.panelUnavailableMissing);
		// The Server closes a Tab's last Panel together with its Tab.
		const closeAvailability = (input: PanelCloseInput) =>
			findCloseTarget(input) === null ? panelMissing : { kind: 'enabled' as const };
		/** What in the active Tab stops a layout command, in the order the command checks it. */
		const layoutBlocker = (minimumPanels: number) => {
			if (activeTab === null)
				return disabled(workbenchMessages.panelUnavailableFocusedPanelRequired);
			if (activeTab.maximizedPanelId !== null) {
				return disabled(workbenchMessages.panelUnavailableMaximized);
			}
			if (panelCount < minimumPanels) {
				return disabled(
					minimumPanels > 2
						? workbenchMessages.panelUnavailableThreeRequired
						: workbenchMessages.panelUnavailableMultipleRequired,
				);
			}
			return null;
		};
		/** Both Panels must remain, and a Panel never trades places with itself. */
		const pairAvailability = (input: {
			readonly panelId: string;
			readonly targetPanelId: string;
		}) => {
			if (findTarget(input.panelId) === null || findTarget(input.targetPanelId) === null) {
				return panelMissing;
			}
			return input.panelId === input.targetPanelId
				? disabled(workbenchMessages.panelUnavailableSameTarget)
				: { kind: 'enabled' as const };
		};
		/** What a shared change names, because it can settle after this Client has moved elsewhere. */
		const sharedTabTarget =
			activeTab === null ? null : { tabId: activeTab.tabId, terminalContextId };
		/** A new terminal follows the directory its terminal reported, and otherwise the authority chooses. */
		const reportedDirectory = (panelId: string) => {
			const panel = findTarget(panelId);
			return panel === null ? undefined : (getDisplayedTerminal(panel).currentCwd ?? undefined);
		};
		const targetAvailability = (panelId: string) =>
			findTarget(panelId) === null ? panelMissing : { kind: 'enabled' as const };
		const resultFor = (transition: PanelWorkspaceTransition | null) => {
			if (transition === null || transition.outcome === 'rejected') {
				return {
					code: transition?.rejection ?? 'active-tab-unavailable',
					kind: 'blocked' as const,
				};
			}
			return transition.outcome === 'unchanged'
				? { kind: 'unchanged' as const }
				: { kind: 'completed' as const };
		};
		const registrations = [
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) => targetAvailability(input.panelId),
					handler: {
						execute: (context, input) => {
							context.markCommitted();
							return resultFor(
								operations.activateChildTerminal(input.panelId, input.childPresentationId),
							);
						},
					},
					id: PANEL_ACTION_IDS.activateChildTerminal,
					input: panelChildTerminalInput,
					title: workbenchMessages.panelActivateChild,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) =>
						layoutBlocker(3) ?? targetAvailability(input.panelId),
					handler: {
						execute: (_context, input) =>
							operations.startPanelVisualSelection(input.panelId, input.mode)
								? { kind: 'completed' }
								: { code: 'stale-target', kind: 'blocked' },
					},
					id: PANEL_ACTION_IDS.startVisualSelection,
					input: panelVisualSelectionInput,
					title: workbenchMessages.panelVisualSelectionStart,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) =>
						layoutBlocker(2) ?? targetAvailability(input.panelId),
					handler: {
						execute: (context) =>
							sharedTabTarget === null
								? { code: 'active-tab-unavailable', kind: 'blocked' }
								: authority.change(context, { ...sharedTabTarget, type: 'panels-equalized' }),
					},
					id: PANEL_ACTION_IDS.equalize,
					input: panelTargetInput,
					title: workbenchMessages.panelEqualize,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) => layoutBlocker(2) ?? pairAvailability(input),
					handler: {
						execute: (context, input) => {
							if (sharedTabTarget === null)
								return { code: 'active-tab-unavailable', kind: 'blocked' };
							// Which Panel holds this Client's focus is not the authority's to say.
							operations.focusPanel(input.panelId);
							return authority.change(context, {
								...sharedTabTarget,
								panelId: input.panelId,
								targetPanelId: input.targetPanelId,
								type: 'panels-swapped',
							});
						},
					},
					id: PANEL_ACTION_IDS.swap,
					input: panelSwapInput,
					title: workbenchMessages.panelMove,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) =>
						layoutBlocker(1) ?? targetAvailability(input.panelId),
					handler: {
						execute: (context, input) =>
							sharedTabTarget === null
								? { code: 'active-tab-unavailable', kind: 'blocked' }
								: authority.change(context, {
										...sharedTabTarget,
										adjustment: input.adjustment,
										multiplier: input.multiplier,
										panelId: input.panelId,
										type: 'panel-size-adjusted',
									}),
					},
					id: PANEL_ACTION_IDS.resize,
					input: panelResizeInput,
					title: workbenchMessages.panelResize,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution: closeExecution,
					getAvailability: (_context, input) => closeAvailability(input),
					// A Panel that runs nothing closes without asking, and only the authority can tell.
					handler: {
						execute: async (context, input) => {
							const result = await authority.close(
								context,
								closureFor(input, 'panel-close-if-idle'),
							);
							if (result.kind !== 'blocked' || result.code !== 'confirmation-required') {
								return result;
							}
							// The question is its own Action, because this one has already asked the authority.
							typedRuntime.execute(PANEL_ACTION_IDS.closeConfirmed, input, {
								scope,
								source: context.source,
								target: { id: input.panelId, type: 'panel' },
							});
							return { kind: 'unchanged' };
						},
					},
					id: PANEL_ACTION_IDS.close,
					input: panelCloseInput,
					title: workbenchMessages.panelClose,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					confirmation: (input) => ({
						cancelLabel: workbenchMessages.panelCancel,
						confirmLabel: workbenchMessages.panelClose,
						defaultAnswer: 'confirm',
						description: {
							...workbenchMessages.terminalCloseWarning,
							values: terminalCloseWarningValues(terminalCloseImpact(closedTerminals(input))),
						},
						risk: 'destructive',
						title: workbenchMessages.panelCloseTitle,
						watch: (withdraw) => authority.watchClose(closureFor(input, 'panel-close'), withdraw),
					}),
					execution: closeExecution,
					getAvailability: (_context, input) => closeAvailability(input),
					handler: {
						execute: (context, input) => authority.close(context, closureFor(input, 'panel-close')),
					},
					id: PANEL_ACTION_IDS.closeConfirmed,
					input: panelCloseInput,
					title: workbenchMessages.panelClose,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) => targetAvailability(input.panelId),
					handler: {
						execute: (_context, input) =>
							operations.startPanelResize(input.panelId)
								? { kind: 'completed' }
								: { code: 'stale-target', kind: 'blocked' },
					},
					id: PANEL_ACTION_IDS.startResize,
					input: panelTargetInput,
					title: workbenchMessages.panelResize,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) => targetAvailability(input.panelId),
					handler: {
						execute: (context, input) => {
							context.markCommitted();
							return resultFor(operations.focusPanel(input.panelId));
						},
					},
					id: PANEL_ACTION_IDS.focus,
					input: panelTargetInput,
					title: workbenchMessages.panelFocus,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) =>
						panelCount < 2
							? disabled(workbenchMessages.panelUnavailableMultipleRequired)
							: pairAvailability(input),
					handler: {
						execute: (context, input) => {
							if (sharedTabTarget === null)
								return { code: 'active-tab-unavailable', kind: 'blocked' };
							// Which Panel holds this Client's focus is not the authority's to say.
							operations.focusPanel(input.panelId);
							return authority.change(context, {
								...sharedTabTarget,
								direction: input.direction,
								panelId: input.panelId,
								targetPanelId: input.targetPanelId,
								type: 'panel-moved',
							});
						},
					},
					id: PANEL_ACTION_IDS.move,
					input: panelMoveInput,
					title: workbenchMessages.panelMove,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) => {
						const panel = findTarget(input.panelId);
						if (panel === null) return panelMissing;
						return panel.activeChildPresentationId === null
							? disabled(workbenchMessages.panelUnavailablePrimaryShown)
							: { kind: 'enabled' };
					},
					handler: {
						execute: (context, input) => {
							context.markCommitted();
							return resultFor(operations.showPrimaryTerminal(input.panelId));
						},
					},
					id: PANEL_ACTION_IDS.showPrimaryTerminal,
					input: panelTargetInput,
					title: workbenchMessages.panelReturnToPrimary,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) => targetAvailability(input.panelId),
					handler: {
						execute: (context, input) =>
							sharedTabTarget === null
								? { code: 'active-tab-unavailable', kind: 'blocked' }
								: createPanelTerminal(context, {
										...sharedTabTarget,
										cwd: reportedDirectory(input.panelId),
										direction: input.direction,
										panelId: input.panelId,
									}),
					},
					id: PANEL_ACTION_IDS.split,
					input: panelSplitInput,
					title: workbenchMessages.panelSplit,
				}),
			),
			typedRuntime.register(
				scope,
				defineAction({
					category,
					execution,
					getAvailability: (_context, input) =>
						panelCount < 2
							? disabled(workbenchMessages.panelUnavailableMultipleRequired)
							: targetAvailability(input.panelId),
					handler: {
						execute: (context, input) => {
							context.markCommitted();
							return resultFor(operations.togglePanelMaximization(input.panelId));
						},
					},
					id: PANEL_ACTION_IDS.toggleMaximization,
					input: panelTargetInput,
					title: workbenchMessages.panelMaximize,
				}),
			),
		];

		return () => {
			for (const registration of registrations) {
				registration.dispose();
			}
		};
	}, [
		activeTab,
		authority,
		createPanelTerminal,
		operations,
		panelCount,
		scope,
		terminalContextId,
		typedRuntime,
	]);

	const execute = useMemoizedFn(
		<Id extends ActionCatalogId<PanelActionCatalog>>(
			actionId: Id,
			input: ActionInputFor<PanelActionCatalog, Id>,
			source: ActionInvocationSource,
		): ActionExecution<ActionOutputFor<PanelActionCatalog, Id>> => {
			const panelId =
				typeof input === 'object' &&
				input !== null &&
				'panelId' in input &&
				typeof input.panelId === 'string'
					? input.panelId
					: null;
			return typedRuntime.execute(actionId, input, {
				scope,
				source,
				...(panelId === null ? {} : { target: { id: panelId, type: 'panel' } }),
			});
		},
	);

	return { execute, scope };
}
