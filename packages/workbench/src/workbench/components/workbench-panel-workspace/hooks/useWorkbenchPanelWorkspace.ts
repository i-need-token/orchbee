import { useOrchbeeIntl, workbenchMessages, workbenchTabMessages } from '@orchbee/i18n';
import type { ActionInvocationSource, ActionRuntime, ActionScope } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import type { TabModel } from '@orchbee/tab-strip';
import * as React from 'react';

import { PanelWorkspaceStore } from '../model/PanelWorkspaceStore';
import {
	PANEL_ACTION_IDS,
	type PanelCloseInput,
	type PanelMoveInput,
	type PanelSplitInput,
	type PanelTargetInput,
	type PanelChildTerminalInput,
} from '../actions/panel-action-contracts';
import {
	collectPanelLeaves,
	findPanel,
	type PanelLayoutNode,
	type PanelSplitDirection,
	type PanelWorkspaceEvent,
	type PanelWorkspaceTransition,
	projectPanelDrag,
} from '../model/panel-workspace-state';
import type {
	PanelDragGhost,
	PanelDropTarget,
	PanelWorkspaceMessages,
	PendingPanelClose,
	WorkbenchPanelWorkspaceView,
} from '../types/panel-workspace-view';
import { createPanelDragSnapshot } from '../utils/createPanelDragSnapshot';
import { getPanelDropDirection } from '../utils/getPanelDropDirection';
import { useWorkbenchPanelActions } from './useWorkbenchPanelActions';

interface UseWorkbenchPanelWorkspaceOptions {
	readonly actionRuntime: ActionRuntime;
	readonly parentActionScope: ActionScope;
	readonly tabModel: TabModel;
}

interface PanelDragSession {
	readonly captureElement: HTMLElement;
	readonly grabOffsetX: number;
	readonly grabOffsetY: number;
	readonly panelId: string;
	readonly pointerId: number;
	readonly previewHeight: number;
	readonly previewScale: number;
	readonly previewWidth: number;
	readonly sourceElement: HTMLElement;
	readonly sourceHeight: number;
	readonly sourceRoot: PanelLayoutNode;
	readonly sourceWidth: number;
	readonly startX: number;
	readonly startY: number;
	readonly workspaceElement: HTMLElement;
}

type ActiveTabPanelWorkspaceEvent = Exclude<PanelWorkspaceEvent, { type: 'tabs-reconciled' }>;
type ActiveTabPanelWorkspaceEventInput = ActiveTabPanelWorkspaceEvent extends infer Event
	? Event extends { readonly tabId: string }
		? Omit<Event, 'tabId'>
		: never
	: never;

const PANEL_DRAG_PREVIEW_MAX_HEIGHT_PX = 192;
const PANEL_DRAG_PREVIEW_MAX_WIDTH_PX = 320;

export function useWorkbenchPanelWorkspace({
	actionRuntime,
	parentActionScope,
	tabModel,
}: UseWorkbenchPanelWorkspaceOptions): WorkbenchPanelWorkspaceView {
	const intl = useOrchbeeIntl();
	const store = React.useMemo(() => new PanelWorkspaceStore(tabModel), [tabModel]);
	const state = React.useSyncExternalStore(store.subscribe, store.getSnapshot, store.getSnapshot);
	const activeTabId = React.useSyncExternalStore(
		store.subscribe,
		store.getActiveTabId,
		store.getActiveTabId,
	);
	const [pendingClose, setPendingClose] = React.useState<PendingPanelClose | null>(null);
	const pendingConfirmationRef = React.useRef<((accepted: boolean) => void) | null>(null);
	const [panelDragSession, setPanelDragSession] = React.useState<PanelDragSession | null>(null);
	const [panelDrag, setPanelDrag] = React.useState<WorkbenchPanelWorkspaceView['panelDrag']>({
		draggingPanelId: null,
		dropTarget: null,
		ghost: null,
		previewRoot: null,
	});
	const activeTab = state.tabs.find((tab) => tab.tabId === activeTabId) ?? null;
	const panelCount = activeTab === null ? 0 : collectPanelLeaves(activeTab.root).length;
	const canonicalDisplayedRoot =
		activeTab?.maximizedPanelId === null || activeTab === null
			? (activeTab?.root ?? null)
			: findPanel(activeTab.root, activeTab.maximizedPanelId);
	const displayedRoot = panelDrag.previewRoot ?? canonicalDisplayedRoot;
	const messages = React.useMemo<PanelWorkspaceMessages>(
		() => ({
			cancel: intl.formatMessage(workbenchMessages.panelCancel),
			childTerminalsLabel: intl.formatMessage(workbenchMessages.panelChildTerminalsLabel),
			close: intl.formatMessage(workbenchMessages.panelClose),
			closeAnyway: intl.formatMessage(workbenchMessages.panelCloseAnyway),
			closeRunningDescription: (count) =>
				intl.formatMessage(workbenchMessages.panelCloseRunningDescription, { count }),
			closeRunningTitle: intl.formatMessage(workbenchMessages.panelCloseRunningTitle),
			maximize: intl.formatMessage(workbenchMessages.panelMaximize),
			minimize: intl.formatMessage(workbenchMessages.panelMinimize),
			more: intl.formatMessage(workbenchMessages.panelMore),
			newChildTerminal: intl.formatMessage(workbenchMessages.panelNewChildTerminal),
			returnToPrimary: intl.formatMessage(workbenchMessages.panelReturnToPrimary),
			splitAbove: intl.formatMessage(workbenchMessages.panelSplitAbove),
			splitBelow: intl.formatMessage(workbenchMessages.panelSplitBelow),
			splitLeft: intl.formatMessage(workbenchMessages.panelSplitLeft),
			splitRight: intl.formatMessage(workbenchMessages.panelSplitRight),
		}),
		[intl],
	);
	const createTerminalTitle = useMemoizedFn(() =>
		intl.formatMessage(workbenchTabMessages.terminalTitle, {
			number: store.getSnapshot().nextTerminalSequence,
		}),
	);
	const transitionForActiveTab = useMemoizedFn(
		(event: ActiveTabPanelWorkspaceEventInput): PanelWorkspaceTransition | null => {
			const tabId = store.getActiveTabId();
			return tabId === null ? null : store.transition({ ...event, tabId });
		},
	);
	const commitMovePanel = useMemoizedFn(
		(panelId: string, targetPanelId: string, direction: PanelSplitDirection) => {
			return transitionForActiveTab({
				direction,
				panelId,
				targetPanelId,
				type: 'panel-moved',
			});
		},
	);
	const operations = React.useMemo(
		() => ({
			activateChildTerminal: (panelId: string, childTerminalId: string) =>
				transitionForActiveTab({
					childTerminalId,
					panelId,
					type: 'child-terminal-activated',
				}),
			closeDisplayedTerminal: (input: PanelCloseInput) => {
				const tabId = store.getActiveTabId();
				const tab = store.getSnapshot().tabs.find((candidate) => candidate.tabId === input.tabId);
				const panel = tab === undefined ? null : findPanel(tab.root, input.panelId);
				if (
					tabId !== input.tabId ||
					panel === null ||
					panel.activeChildTerminalId !== input.childTerminalId
				) {
					return null;
				}
				return store.transition(
					input.childTerminalId === null
						? { panelId: input.panelId, tabId: input.tabId, type: 'panel-closed' }
						: {
								childTerminalId: input.childTerminalId,
								panelId: input.panelId,
								tabId: input.tabId,
								type: 'child-terminal-closed',
							},
				);
			},
			createChildTerminal: (panelId: string) =>
				transitionForActiveTab({
					newTerminalTitle: createTerminalTitle(),
					panelId,
					type: 'child-terminal-created',
				}),
			focusPanel: (panelId: string) => transitionForActiveTab({ panelId, type: 'panel-focused' }),
			movePanel: commitMovePanel,
			showPrimaryTerminal: (panelId: string) =>
				transitionForActiveTab({ panelId, type: 'primary-terminal-shown' }),
			splitPanel: (panelId: string, direction: PanelSplitDirection) => {
				const tabId = store.getActiveTabId();
				if (tabId === null) {
					return null;
				}
				const newPanelId = `panel-${store.getSnapshot().nextPanelSequence}`;
				return {
					panelId: newPanelId,
					transition: store.transition({
						direction,
						newTerminalTitle: createTerminalTitle(),
						panelId,
						tabId,
						type: 'panel-split',
					}),
				};
			},
			togglePanelMaximization: (panelId: string) =>
				transitionForActiveTab({ panelId, type: 'panel-maximization-toggled' }),
		}),
		[commitMovePanel, createTerminalTitle, store, transitionForActiveTab],
	);
	const panelActions = useWorkbenchPanelActions({
		activeTab,
		operations,
		panelCount,
		parentScope: parentActionScope,
		runtime: actionRuntime,
	});
	const executePanelAction = panelActions.execute;
	React.useEffect(() => {
		const registration = actionRuntime.registerConfirmationPresenter(
			panelActions.scope,
			(confirmation, invocation, signal) => {
				const panelId = invocation.target?.type === 'panel' ? invocation.target.id : null;
				const runningCount = confirmation.description.values?.['count'];
				if (panelId === null || typeof runningCount !== 'number') {
					return false;
				}
				return new Promise<boolean>((resolve) => {
					pendingConfirmationRef.current?.(false);
					let settled = false;
					const finish = (accepted: boolean) => {
						if (settled) {
							return;
						}
						settled = true;
						signal.removeEventListener('abort', onAbort);
						if (pendingConfirmationRef.current === finish) {
							pendingConfirmationRef.current = null;
							setPendingClose(null);
						}
						resolve(accepted);
					};
					const onAbort = () => finish(false);
					pendingConfirmationRef.current = finish;
					setPendingClose({ panelId, runningCount });
					signal.addEventListener('abort', onAbort, { once: true });
				});
			},
		);
		return () => {
			registration.dispose();
			pendingConfirmationRef.current?.(false);
		};
	}, [actionRuntime, panelActions.scope, store]);
	const movePanel = useMemoizedFn(
		(panelId: string, targetPanelId: string, direction: PanelSplitDirection) => {
			executePanelAction(
				PANEL_ACTION_IDS.move,
				{ direction, panelId, targetPanelId } satisfies PanelMoveInput,
				'pointer',
			);
		},
	);
	const startPanelDrag = useMemoizedFn(
		(panelId: string, event: React.PointerEvent<HTMLElement>) => {
			if (
				event.button !== 0 ||
				panelDragSession !== null ||
				activeTab === null ||
				activeTab.maximizedPanelId !== null ||
				panelCount < 2
			) {
				return;
			}
			const panel = findPanel(activeTab.root, panelId);
			const workspaceElement = event.currentTarget.closest(
				'[data-slot="workbench-panel-workspace"]',
			);
			const sourceElement = event.currentTarget.closest('[data-slot="workbench-panel"]');
			if (
				panel === null ||
				panel.activeChildTerminalId !== null ||
				!(sourceElement instanceof HTMLElement) ||
				!(workspaceElement instanceof HTMLElement)
			) {
				return;
			}
			const sourceBounds = sourceElement.getBoundingClientRect();
			if (sourceBounds.width <= 0 || sourceBounds.height <= 0) {
				return;
			}
			const previewScale = Math.min(1, PANEL_DRAG_PREVIEW_MAX_WIDTH_PX / sourceBounds.width);
			const previewHeight = Math.min(
				sourceBounds.height * previewScale,
				PANEL_DRAG_PREVIEW_MAX_HEIGHT_PX,
			);
			event.preventDefault();
			workspaceElement.setPointerCapture(event.pointerId);
			setPanelDragSession({
				captureElement: workspaceElement,
				grabOffsetX: (event.clientX - sourceBounds.left) * previewScale,
				grabOffsetY: (event.clientY - sourceBounds.top) * previewScale,
				panelId,
				pointerId: event.pointerId,
				previewHeight,
				previewScale,
				previewWidth: sourceBounds.width * previewScale,
				sourceElement,
				sourceHeight: sourceBounds.height,
				sourceRoot: activeTab.root,
				sourceWidth: sourceBounds.width,
				startX: event.clientX,
				startY: event.clientY,
				workspaceElement,
			});
		},
	);

	React.useEffect(() => {
		if (panelDragSession === null) {
			return;
		}
		const ownerDocument = panelDragSession.captureElement.ownerDocument;
		let dragStarted = false;
		let dropTarget: PanelDropTarget | null = null;
		let finished = false;
		let mountSnapshot: PanelDragGhost['mountSnapshot'] | null = null;
		const resetProjection = () => {
			setPanelDrag({
				draggingPanelId: null,
				dropTarget: null,
				ghost: null,
				previewRoot: null,
			});
		};
		const finishDrag = (commit: boolean) => {
			if (finished) {
				return;
			}
			finished = true;
			if (commit && dragStarted && dropTarget !== null) {
				movePanel(panelDragSession.panelId, dropTarget.panelId, dropTarget.direction);
			}
			resetProjection();
			setPanelDragSession(null);
			if (panelDragSession.captureElement.hasPointerCapture(panelDragSession.pointerId)) {
				panelDragSession.captureElement.releasePointerCapture(panelDragSession.pointerId);
			}
		};
		const handlePointerMove = (event: PointerEvent) => {
			if (event.pointerId !== panelDragSession.pointerId) {
				return;
			}
			if (
				!dragStarted &&
				Math.hypot(
					event.clientX - panelDragSession.startX,
					event.clientY - panelDragSession.startY,
				) < 4
			) {
				return;
			}
			dragStarted = true;
			if (mountSnapshot === null) {
				const snapshot = createPanelDragSnapshot(panelDragSession.sourceElement);
				if (snapshot === null) {
					finishDrag(false);
					return;
				}
				mountSnapshot = snapshot.mount;
			}
			const hitElement = ownerDocument.elementFromPoint(event.clientX, event.clientY);
			const panelElement = hitElement?.closest('[data-slot="workbench-panel"]');
			const panelId =
				panelElement instanceof HTMLElement ? panelElement.dataset['panelId'] : undefined;
			const isWorkspaceTarget =
				panelElement?.closest('[data-slot="workbench-panel-workspace"]') ===
				panelDragSession.workspaceElement;
			const isCurrentPlaceholder =
				panelElement instanceof HTMLElement &&
				panelId === panelDragSession.panelId &&
				panelElement.dataset['dropPlaceholder'] === 'true' &&
				dropTarget !== null;
			if (!isCurrentPlaceholder) {
				const direction =
					panelElement instanceof HTMLElement &&
					isWorkspaceTarget &&
					panelId !== undefined &&
					panelId !== panelDragSession.panelId
						? getPanelDropDirection(panelElement.getBoundingClientRect(), {
								x: event.clientX,
								y: event.clientY,
							})
						: null;
				dropTarget = direction === null || panelId === undefined ? null : { direction, panelId };
			}
			const previewRoot = projectPanelDrag(
				panelDragSession.sourceRoot,
				panelDragSession.panelId,
				dropTarget === null
					? null
					: {
							direction: dropTarget.direction,
							targetPanelId: dropTarget.panelId,
						},
			);
			setPanelDrag({
				draggingPanelId: panelDragSession.panelId,
				dropTarget,
				ghost: {
					blockStart: event.clientY - panelDragSession.grabOffsetY,
					height: panelDragSession.previewHeight,
					inlineStart: event.clientX - panelDragSession.grabOffsetX,
					mountSnapshot,
					scale: panelDragSession.previewScale,
					sourceHeight: panelDragSession.sourceHeight,
					sourceWidth: panelDragSession.sourceWidth,
					width: panelDragSession.previewWidth,
				},
				previewRoot,
			});
			event.preventDefault();
		};
		const handlePointerUp = (event: PointerEvent) => {
			if (event.pointerId === panelDragSession.pointerId) {
				finishDrag(true);
			}
		};
		const handlePointerCancel = (event: PointerEvent) => {
			if (event.pointerId === panelDragSession.pointerId) {
				finishDrag(false);
			}
		};
		const handleLostPointerCapture = () => {
			finishDrag(false);
		};
		ownerDocument.addEventListener('pointermove', handlePointerMove, { passive: false });
		ownerDocument.addEventListener('pointerup', handlePointerUp);
		ownerDocument.addEventListener('pointercancel', handlePointerCancel);
		panelDragSession.captureElement.addEventListener(
			'lostpointercapture',
			handleLostPointerCapture,
		);
		return () => {
			ownerDocument.removeEventListener('pointermove', handlePointerMove);
			ownerDocument.removeEventListener('pointerup', handlePointerUp);
			ownerDocument.removeEventListener('pointercancel', handlePointerCancel);
			panelDragSession.captureElement.removeEventListener(
				'lostpointercapture',
				handleLostPointerCapture,
			);
			if (panelDragSession.captureElement.hasPointerCapture(panelDragSession.pointerId)) {
				panelDragSession.captureElement.releasePointerCapture(panelDragSession.pointerId);
			}
		};
	}, [movePanel, panelDragSession]);

	return {
		activateChildTerminal: (
			panelId,
			childTerminalId,
			source: ActionInvocationSource = 'programmatic',
		) => {
			executePanelAction(
				PANEL_ACTION_IDS.activateChildTerminal,
				{ childTerminalId, panelId } satisfies PanelChildTerminalInput,
				source,
			);
		},
		activeTab,
		cancelClose: () => pendingConfirmationRef.current?.(false),
		confirmClose: () => {
			pendingConfirmationRef.current?.(true);
		},
		createChildTerminal: (panelId, source: ActionInvocationSource = 'programmatic') => {
			executePanelAction(
				PANEL_ACTION_IDS.createChildTerminal,
				{ panelId } satisfies PanelTargetInput,
				source,
			);
		},
		displayedRoot,
		focusPanel: (panelId, source: ActionInvocationSource = 'programmatic') => {
			executePanelAction(PANEL_ACTION_IDS.focus, { panelId } satisfies PanelTargetInput, source);
		},
		maximizedPanelId: activeTab?.maximizedPanelId ?? null,
		messages,
		panelCount,
		panelDrag,
		pendingClose,
		requestClose: (panelId, source: ActionInvocationSource = 'programmatic') => {
			const tabId = store.getActiveTabId();
			const tab = store.getSnapshot().tabs.find((candidate) => candidate.tabId === tabId);
			const panel = tab === undefined ? null : findPanel(tab.root, panelId);
			if (tabId === null || panel === null) {
				return;
			}
			executePanelAction(
				PANEL_ACTION_IDS.close,
				{
					childTerminalId: panel.activeChildTerminalId,
					panelId,
					tabId,
				} satisfies PanelCloseInput,
				source,
			);
		},
		resizeAdjacentPanels: (splitId, separatorIndex, leadingWeight) => {
			transitionForActiveTab({
				leadingWeight,
				separatorIndex,
				splitId,
				type: 'adjacent-panels-resized',
			});
		},
		showPrimaryTerminal: (panelId, source: ActionInvocationSource = 'programmatic') => {
			executePanelAction(
				PANEL_ACTION_IDS.showPrimaryTerminal,
				{ panelId } satisfies PanelTargetInput,
				source,
			);
		},
		splitPanel: (panelId, direction, source: ActionInvocationSource = 'programmatic') => {
			const execution = executePanelAction(
				PANEL_ACTION_IDS.split,
				{ direction, panelId } satisfies PanelSplitInput,
				source,
			);
			const result = execution.getSnapshot().result;
			return result?.kind === 'completed' && typeof result.value === 'string' ? result.value : null;
		},
		startPanelDrag,
		state,
		togglePanelMaximization: (panelId, source: ActionInvocationSource = 'programmatic') => {
			executePanelAction(
				PANEL_ACTION_IDS.toggleMaximization,
				{ panelId } satisfies PanelTargetInput,
				source,
			);
		},
	};
}
