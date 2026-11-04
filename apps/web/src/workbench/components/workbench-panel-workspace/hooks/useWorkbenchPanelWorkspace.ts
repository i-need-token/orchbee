import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import type { ActionInvocationSource, ActionRuntime, ActionScope } from '@orchbee/actions';
import { useMemoizedFn } from '@orchbee/foundation/react';
import * as React from 'react';
import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

import type { WorkbenchSharedPanelTerminalCreate } from '../../../actions/createSharedPanelTerminal';
import { WORKBENCH_ACTION_IDS } from '../../../actions/workbench-action-ids';
import { terminalCloseWarningValues } from '../../../model/terminal-close-impact';
import {
	WorkbenchTerminalBellEffectCoordinator,
	type WorkbenchTerminalBellEffect,
} from '../../../model/WorkbenchTerminalBellEffectCoordinator';
import type {
	WorkbenchAuthority,
	WorkbenchAuthorityContext,
} from '../../../types/workbench-authority';
import type { PanelWorkspaceStore } from '../model/PanelWorkspaceStore';
import {
	PANEL_ACTION_IDS,
	type PanelCloseInput,
	type PanelChildTerminalInput,
	type PanelMoveInput,
	type PanelResizeInput,
	type PanelSplitInput,
	type PanelSwapInput,
	type PanelTargetInput,
	type PanelVisualSelectionInput,
} from '../actions/panel-action-contracts';
import {
	collectPanelLeaves,
	findPanel,
	getDisplayedTerminal,
	getTerminalDisplayTitle,
	type PanelLayoutNode,
	type PanelSplitDirection,
	type PanelWorkspaceEvent,
	type PanelWorkspaceTransition,
	projectPanelDrag,
} from '../model/panel-workspace-state';
import {
	startPanelVisualSelection,
	transitionPanelVisualSelection,
	type PanelVisualSelectionCommand,
	type PanelVisualSelectionSession,
	type PanelVisualSelectionTransition,
} from '../model/panel-visual-selection';
import { projectTerminalTabPresentationRevision } from '../model/projectTerminalTabPresentationRevision';
import { resolvePanelResize } from '../model/resolvePanelResize';
import type {
	PanelDragGhost,
	PanelDropTarget,
	PanelWorkspaceMessages,
	PendingPanelClose,
	PendingPanelResize,
	SharedSplitResize,
	WorkbenchPanelWorkspaceView,
} from '../types/panel-workspace-view';
import { createPanelDragSnapshot } from '../utils/createPanelDragSnapshot';
import { getPanelDropDirection } from '../utils/getPanelDropDirection';
import { useWorkbenchPanelActions } from './useWorkbenchPanelActions';

interface UseWorkbenchPanelWorkspaceOptions {
	readonly onTerminalEdit?: ((presentationId: string) => void) | undefined;
	readonly actionRuntime: ActionRuntime;
	readonly authority: WorkbenchAuthority;
	readonly createPanelTerminal: WorkbenchSharedPanelTerminalCreate;
	readonly panelWorkspaceStore: PanelWorkspaceStore;
	readonly parentActionScope: ActionScope;
	readonly shortcutLabelForAction: (actionId: string) => string | undefined;
	readonly terminalContextId: string;
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
	readonly sourceRevision: string;
	readonly sourceRoot: PanelLayoutNode;
	readonly sourceWidth: number;
	readonly startX: number;
	readonly startY: number;
	readonly tabId: string;
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
const EMPTY_PANEL_DRAG: WorkbenchPanelWorkspaceView['panelDrag'] = Object.freeze({
	draggingPanelId: null,
	dropTarget: null,
	ghost: null,
	previewRoot: null,
	sourceRevision: null,
});
/** A divider's release is not an Action anyone can cancel: it settles once, with what it submitted. */
const SPLIT_RATIO_CONTEXT: WorkbenchAuthorityContext = {
	markCommitted: () => undefined,
	signal: new AbortController().signal,
};

export function useWorkbenchPanelWorkspace({
	onTerminalEdit,
	actionRuntime,
	authority,
	createPanelTerminal,
	panelWorkspaceStore,
	parentActionScope,
	shortcutLabelForAction,
	terminalContextId,
}: UseWorkbenchPanelWorkspaceOptions): WorkbenchPanelWorkspaceView {
	const intl = useOrchbeeIntl();
	const workspaceReference = React.useRef<HTMLDivElement>(null);
	const [terminalBellEffectCoordinator] = React.useState(
		() => new WorkbenchTerminalBellEffectCoordinator(),
	);
	const store = panelWorkspaceStore;
	const state = React.useSyncExternalStore(store.subscribe, store.getSnapshot, store.getSnapshot);
	const activeTabId = React.useSyncExternalStore(
		store.subscribe,
		store.getActiveTabId,
		store.getActiveTabId,
	);
	const [pendingClose, setPendingClose] = React.useState<PendingPanelClose | null>(null);
	const [pendingResize, setPendingResize] = React.useState<PendingPanelResize | null>(null);
	const [panelVisualSelection, setPanelVisualSelection] =
		React.useState<PanelVisualSelectionSession | null>(null);
	const pendingPanelFocusRestoreIdRef = React.useRef<string | null>(null);
	const pendingConfirmationRef = React.useRef<((accepted: boolean) => void) | null>(null);
	const [panelDragSession, setPanelDragSession] = React.useState<PanelDragSession | null>(null);
	const [panelDrag, setPanelDrag] =
		React.useState<WorkbenchPanelWorkspaceView['panelDrag']>(EMPTY_PANEL_DRAG);
	const admitTerminalBellEffect = useMemoizedFn((event: WorkbenchTerminalBellEffect): boolean =>
		terminalBellEffectCoordinator.admit(event),
	);
	const activeTab = state.tabs.find((tab) => tab.tabId === activeTabId) ?? null;
	const panelCount = activeTab === null ? 0 : collectPanelLeaves(activeTab.root).length;
	const displayedRoot = panelDrag.previewRoot ?? activeTab?.root ?? null;
	// A drag places a Panel among the ones it started from. A publication that only changes sizes or
	// names leaves those in place, and anything else leaves nothing for it to mean.
	React.useLayoutEffect(() => {
		if (
			panelDragSession === null ||
			(panelDragSession.tabId === activeTabId &&
				panelDragSession.sourceRevision ===
					projectTerminalTabPresentationRevision(terminalContextId, activeTab))
		) {
			return;
		}
		setPanelDragSession(null);
		setPanelDrag(EMPTY_PANEL_DRAG);
	}, [activeTab, activeTabId, panelDragSession, terminalContextId]);
	React.useLayoutEffect(() => {
		if (pendingResize === null) return;
		const tab = state.tabs.find((candidate) => candidate.tabId === pendingResize.tabId);
		if (
			activeTabId !== pendingResize.tabId ||
			tab === undefined ||
			findPanel(tab.root, pendingResize.panelId) === null
		) {
			setPendingResize(null);
		}
	}, [activeTabId, pendingResize, state.tabs]);
	const messages = React.useMemo<PanelWorkspaceMessages>(
		() => ({
			cancel: intl.formatMessage(workbenchMessages.panelCancel),
			childPresentationsLabel: intl.formatMessage(workbenchMessages.panelChildTerminalsLabel),
			close: intl.formatMessage(workbenchMessages.panelClose),
			closeLastPanel: intl.formatMessage(workbenchMessages.panelCloseLastPanel),
			closeTab: (tab) => intl.formatMessage(workbenchMessages.panelCloseTab, { tab }),
			closeTitle: intl.formatMessage(workbenchMessages.panelCloseTitle),
			closeWarning: (impact) =>
				intl.formatMessage(
					workbenchMessages.terminalCloseWarning,
					terminalCloseWarningValues(impact),
				),
			maximize: intl.formatMessage(workbenchMessages.panelMenuMaximize),
			more: intl.formatMessage(workbenchMessages.panelMore),
			restore: intl.formatMessage(workbenchMessages.panelMenuRestore),
			returnToPrimary: intl.formatMessage(workbenchMessages.panelReturnToPrimary),
			resizeDescription: intl.formatMessage(workbenchMessages.panelResizeDescription),
			resizeDone: intl.formatMessage(workbenchMessages.panelResizeDone),
			resizeNarrower: intl.formatMessage(workbenchMessages.panelResizeNarrower),
			resizeReset: intl.formatMessage(workbenchMessages.panelResizeReset),
			resizeShorter: intl.formatMessage(workbenchMessages.panelResizeShorter),
			resizeTaller: intl.formatMessage(workbenchMessages.panelResizeTaller),
			resizeTitle: intl.formatMessage(workbenchMessages.panelResizeTitle),
			resizeUnavailable: intl.formatMessage(workbenchMessages.panelResizeUnavailable),
			resizeWider: intl.formatMessage(workbenchMessages.panelResizeWider),
			splitBelow: intl.formatMessage(workbenchMessages.panelSplitBelow),
			splitRight: intl.formatMessage(workbenchMessages.panelSplitRight),
			visualSelectionFocusLabel: intl.formatMessage(
				workbenchMessages.panelVisualSelectionFocusLabel,
			),
			visualSelectionFocusTarget: (label) =>
				intl.formatMessage(workbenchMessages.panelVisualSelectionFocusTarget, { label }),
			visualSelectionSwapLabel: intl.formatMessage(workbenchMessages.panelVisualSelectionSwapLabel),
			visualSelectionSwapTarget: (label) =>
				intl.formatMessage(workbenchMessages.panelVisualSelectionSwapTarget, { label }),
		}),
		[intl],
	);
	const transitionForActiveTab = useMemoizedFn(
		(event: ActiveTabPanelWorkspaceEventInput): PanelWorkspaceTransition | null => {
			const tabId = store.getActiveTabId();
			return tabId === null ? null : store.transition({ ...event, tabId });
		},
	);
	const operations = React.useMemo(
		() => ({
			activateChildTerminal: (panelId: string, childPresentationId: TerminalPresentationId) =>
				transitionForActiveTab({
					childPresentationId,
					panelId,
					type: 'child-terminal-activated',
				}),
			focusPanel: (panelId: string) => transitionForActiveTab({ panelId, type: 'panel-focused' }),
			showPrimaryTerminal: (panelId: string) =>
				transitionForActiveTab({ panelId, type: 'primary-terminal-shown' }),
			startPanelResize: (panelId: string) => {
				if (panelVisualSelection !== null) {
					return false;
				}
				const tabId = store.getActiveTabId();
				const tab = store.getSnapshot().tabs.find((candidate) => candidate.tabId === tabId);
				if (tabId === null || tab === undefined || findPanel(tab.root, panelId) === null) {
					return false;
				}
				setPendingResize({ panelId, tabId });
				return true;
			},
			startPanelVisualSelection: (panelId: string, mode: PanelVisualSelectionInput['mode']) => {
				if (
					pendingClose !== null ||
					pendingResize !== null ||
					panelDragSession !== null ||
					panelVisualSelection !== null
				) {
					return false;
				}
				const tabId = store.getActiveTabId();
				const tab = store.getSnapshot().tabs.find((candidate) => candidate.tabId === tabId);
				if (tabId === null || tab === undefined) {
					return false;
				}
				const result = startPanelVisualSelection({
					maximizedPanelId: tab.maximizedPanelId,
					mode,
					root: tab.root,
					sourcePanelId: panelId,
					tabId,
				});
				if (result.kind !== 'started') {
					return false;
				}
				setPanelVisualSelection(result.session);
				return true;
			},
			togglePanelMaximization: (panelId: string) =>
				transitionForActiveTab({ panelId, type: 'panel-maximization-toggled' }),
		}),
		[
			panelDragSession,
			panelVisualSelection,
			pendingClose,
			pendingResize,
			store,
			transitionForActiveTab,
		],
	);
	const panelActions = useWorkbenchPanelActions({
		activeTab,
		authority,
		createPanelTerminal,
		operations,
		panelCount,
		parentScope: parentActionScope,
		runtime: actionRuntime,
		terminalContextId,
	});
	const executePanelAction = panelActions.execute;
	const commitSplitRatio = useMemoizedFn(
		async (splitId: string, leadingWeight: number): Promise<void> => {
			const tabId = store.getActiveTabId();
			if (tabId === null) return;
			await authority.change(SPLIT_RATIO_CONTEXT, {
				leadingWeight,
				splitId,
				tabId,
				terminalContextId,
				type: 'split-ratio-set',
			});
		},
	);
	const minimumSplitPercent = authority.minimumSplitPercent;
	const sharedSplitResize = React.useMemo<SharedSplitResize>(
		() => ({ commit: commitSplitRatio, minimumChildPercent: minimumSplitPercent }),
		[commitSplitRatio, minimumSplitPercent],
	);
	const revalidatePanelVisualSelection = useMemoizedFn(
		(session: PanelVisualSelectionSession): PanelVisualSelectionTransition => {
			const currentActiveTabId = store.getActiveTabId();
			const tab = store
				.getSnapshot()
				.tabs.find((candidate) => candidate.tabId === currentActiveTabId);
			return transitionPanelVisualSelection(session, {
				activeTabId: currentActiveTabId,
				focusedPanelId: tab?.focusedPanelId ?? null,
				maximizedPanelId: tab?.maximizedPanelId ?? null,
				root: tab?.root ?? null,
				type: 'workspace-revalidated',
			});
		},
	);
	const executePanelVisualSelectionCommand = useMemoizedFn(
		(command: PanelVisualSelectionCommand, source: 'keyboard' | 'pointer'): boolean => {
			const execution =
				command.kind === 'focus'
					? executePanelAction(
							PANEL_ACTION_IDS.focus,
							{ panelId: command.targetPanelId } satisfies PanelTargetInput,
							source,
						)
					: executePanelAction(
							PANEL_ACTION_IDS.swap,
							{
								panelId: command.sourcePanelId,
								targetPanelId: command.targetPanelId,
							} satisfies PanelSwapInput,
							source,
						);
			const result = execution.getSnapshot().result;
			// Still with the authority, which reports its own refusal, is not a failure yet.
			return result === null || result.kind === 'completed' || result.kind === 'unchanged';
		},
	);
	const applyPanelVisualSelectionTransition = useMemoizedFn(
		(
			session: PanelVisualSelectionSession,
			transition: PanelVisualSelectionTransition,
			source: 'keyboard' | 'pointer',
		): void => {
			if (transition.kind === 'active') {
				if (transition.outcome === 'updated') {
					setPanelVisualSelection(transition.session);
				}
				return;
			}

			let restorePanelId = transition.restorePanelId;
			if (transition.command !== null) {
				const revalidation = revalidatePanelVisualSelection(session);
				if (
					revalidation.kind === 'closed' ||
					!executePanelVisualSelectionCommand(transition.command, source)
				) {
					restorePanelId = null;
				}
			}
			pendingPanelFocusRestoreIdRef.current = restorePanelId;
			setPanelVisualSelection(null);
		},
	);
	const cancelPanelVisualSelection = useMemoizedFn((restoreSourceFocus: boolean): void => {
		if (panelVisualSelection === null) {
			return;
		}
		applyPanelVisualSelectionTransition(
			panelVisualSelection,
			transitionPanelVisualSelection(panelVisualSelection, {
				restoreSourceFocus,
				type: 'cancelled',
			}),
			'keyboard',
		);
	});
	const handlePanelVisualSelectionKeyDown = useMemoizedFn(
		(event: React.KeyboardEvent<HTMLDivElement>): void => {
			if (panelVisualSelection === null) {
				return;
			}
			event.preventDefault();
			event.stopPropagation();
			applyPanelVisualSelectionTransition(
				panelVisualSelection,
				transitionPanelVisualSelection(panelVisualSelection, {
					code: event.code,
					isComposing: event.nativeEvent.isComposing,
					repeat: event.repeat,
					type: 'key-pressed',
				}),
				'keyboard',
			);
		},
	);
	const handlePanelVisualSelectionPointerDown = useMemoizedFn(
		(event: React.PointerEvent<HTMLDivElement>): void => {
			if (panelVisualSelection === null) {
				return;
			}
			event.preventDefault();
			event.stopPropagation();
			const targetElement =
				event.target instanceof Element
					? event.target.closest('[data-panel-visual-selection-target="true"]')
					: null;
			const targetPanelId =
				targetElement instanceof HTMLElement ? targetElement.dataset['panelId'] : undefined;
			const transition =
				event.button === 0 && targetPanelId !== undefined
					? transitionPanelVisualSelection(panelVisualSelection, {
							panelId: targetPanelId,
							type: 'target-selected',
						})
					: transitionPanelVisualSelection(panelVisualSelection, {
							restoreSourceFocus: true,
							type: 'cancelled',
						});
			applyPanelVisualSelectionTransition(panelVisualSelection, transition, 'pointer');
		},
	);

	React.useLayoutEffect(() => {
		if (panelVisualSelection === null) {
			return;
		}
		if (pendingClose !== null || pendingResize !== null || panelDragSession !== null) {
			setPanelVisualSelection(null);
			return;
		}
		const revalidation = revalidatePanelVisualSelection(panelVisualSelection);
		if (revalidation.kind === 'closed') {
			setPanelVisualSelection(null);
		}
	}, [
		activeTabId,
		panelDragSession,
		panelVisualSelection,
		pendingClose,
		pendingResize,
		revalidatePanelVisualSelection,
		state.tabs,
	]);
	React.useLayoutEffect(() => {
		if (panelVisualSelection !== null) {
			workspaceReference.current?.focus({ preventScroll: true });
		}
	}, [panelVisualSelection, workspaceReference]);
	React.useLayoutEffect(() => {
		const panelId = pendingPanelFocusRestoreIdRef.current;
		if (panelVisualSelection !== null || panelId === null) {
			return;
		}
		pendingPanelFocusRestoreIdRef.current = null;
		const workspaceElement = workspaceReference.current;
		if (workspaceElement === null) {
			return;
		}
		const viewports = workspaceElement.querySelectorAll<HTMLElement>(
			'[data-presentation-mode="active"] [data-slot="workbench-terminal-viewport"]',
		);
		const viewport = [...viewports].find((element) => element.dataset['panelId'] === panelId);
		const focusTarget =
			viewport?.querySelector<HTMLElement>('textarea, [role="textbox"]') ?? viewport;
		focusTarget?.focus({ preventScroll: true });
	}, [panelVisualSelection, workspaceReference]);
	React.useEffect(() => {
		if (panelVisualSelection === null) {
			return;
		}
		const workspaceElement = workspaceReference.current;
		if (workspaceElement === null) {
			return;
		}
		const ownerDocument = workspaceElement.ownerDocument;
		const handlePointerDown = (event: PointerEvent) => {
			if (!(event.target instanceof Node) || workspaceElement.contains(event.target)) {
				return;
			}
			cancelPanelVisualSelection(false);
		};
		const handleWindowBlur = () => cancelPanelVisualSelection(false);
		ownerDocument.addEventListener('pointerdown', handlePointerDown, true);
		ownerDocument.defaultView?.addEventListener('blur', handleWindowBlur);
		return () => {
			ownerDocument.removeEventListener('pointerdown', handlePointerDown, true);
			ownerDocument.defaultView?.removeEventListener('blur', handleWindowBlur);
		};
	}, [cancelPanelVisualSelection, panelVisualSelection, workspaceReference]);
	React.useEffect(() => {
		const registration = actionRuntime.registerConfirmationPresenter(
			panelActions.scope,
			(confirmation, invocation, signal) => {
				const tab = store
					.getSnapshot()
					.tabs.find((candidate) => candidate.tabId === store.getActiveTabId());
				const panel =
					tab === undefined || invocation.target?.type !== 'panel'
						? null
						: findPanel(tab.root, invocation.target.id);
				const tabTitle = tab === undefined ? null : store.getTabTitle(tab.tabId);
				const ended = confirmation.description.values?.['ended'];
				const running = confirmation.description.values?.['running'];
				if (
					tab === undefined ||
					panel === null ||
					tabTitle === null ||
					typeof ended !== 'number' ||
					typeof running !== 'number'
				) {
					return false;
				}
				const close: PendingPanelClose = {
					dissolvesTab: collectPanelLeaves(tab.root).length === 1,
					impact: { ended, running },
					panelId: panel.id,
					tabTitle,
					target: getTerminalDisplayTitle(getDisplayedTerminal(panel)),
				};
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
					setPendingClose(close);
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
		(panelId: string, targetPanelId: string, direction: PanelSplitDirection) =>
			executePanelAction(
				PANEL_ACTION_IDS.move,
				{ direction, panelId, targetPanelId } satisfies PanelMoveInput,
				'pointer',
			),
	);
	const readActiveTabId = useMemoizedFn(() => store.getActiveTabId());
	const releasePanelDragHold = useMemoizedFn((sourceRevision: string) => {
		setPanelDrag((current) =>
			current.sourceRevision === sourceRevision ? EMPTY_PANEL_DRAG : current,
		);
	});
	const startPanelDrag = useMemoizedFn(
		(panelId: string, event: React.PointerEvent<HTMLElement>) => {
			const sourceRevision = projectTerminalTabPresentationRevision(terminalContextId, activeTab);
			if (
				event.button !== 0 ||
				panelDragSession !== null ||
				panelDrag.draggingPanelId !== null ||
				panelVisualSelection !== null ||
				activeTab === null ||
				activeTab.maximizedPanelId !== null ||
				sourceRevision === null ||
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
				panel.activeChildPresentationId !== null ||
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
				sourceRevision,
				sourceRoot: activeTab.root,
				sourceWidth: sourceBounds.width,
				startX: event.clientX,
				startY: event.clientY,
				tabId: activeTab.tabId,
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
		let previewRoot: PanelLayoutNode | null = null;
		/** Whether the drop stays on screen, because an authority still has to say what became of it. */
		const submitDrop = (): boolean => {
			if (
				!dragStarted ||
				dropTarget === null ||
				// The preview of a Panel already at its target is the layout it started from.
				previewRoot === panelDragSession.sourceRoot ||
				readActiveTabId() !== panelDragSession.tabId
			) {
				return false;
			}
			const execution = movePanel(
				panelDragSession.panelId,
				dropTarget.panelId,
				dropTarget.direction,
			);
			const { panelId, sourceRevision } = panelDragSession;
			setPanelDrag({
				draggingPanelId: panelId,
				dropTarget: null,
				ghost: null,
				previewRoot,
				sourceRevision,
			});
			void execution.completion.then((result) => {
				// A committed move ends the hold when the layer it replaces is gone.
				if (result.kind !== 'completed') {
					releasePanelDragHold(sourceRevision);
				}
			});
			return true;
		};
		const finishDrag = (commit: boolean) => {
			if (finished) {
				return;
			}
			finished = true;
			if (!commit || !submitDrop()) {
				setPanelDrag(EMPTY_PANEL_DRAG);
			}
			setPanelDragSession(null);
			if (panelDragSession.captureElement.hasPointerCapture(panelDragSession.pointerId)) {
				panelDragSession.captureElement.releasePointerCapture(panelDragSession.pointerId);
			}
		};
		const handlePointerMove = (event: PointerEvent) => {
			if (event.pointerId !== panelDragSession.pointerId) {
				return;
			}
			if (readActiveTabId() !== panelDragSession.tabId) {
				finishDrag(false);
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
			previewRoot = projectPanelDrag(
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
				sourceRevision: panelDragSession.sourceRevision,
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
	}, [authority, movePanel, panelDragSession, readActiveTabId, releasePanelDragHold]);
	let panelVisualSelectionLabel: string | undefined;
	if (panelVisualSelection?.mode === 'focus') {
		panelVisualSelectionLabel = messages.visualSelectionFocusLabel;
	} else if (panelVisualSelection?.mode === 'swap') {
		panelVisualSelectionLabel = messages.visualSelectionSwapLabel;
	}

	return {
		actionRuntime,
		actionScope: panelActions.scope,
		admitTerminalBellEffect,
		activateChildTerminal: (
			panelId,
			childPresentationId,
			source: ActionInvocationSource = 'programmatic',
		) => {
			executePanelAction(
				PANEL_ACTION_IDS.activateChildTerminal,
				{ childPresentationId, panelId } satisfies PanelChildTerminalInput,
				source,
			);
		},
		activeTab,
		onTerminalEdit,
		cancelPanelVisualSelection,
		cancelPanelResize: () => setPendingResize(null),
		canResizePanel: (panelId, adjustment, multiplier = 1) => {
			const tabId = store.getActiveTabId();
			const tab = store.getSnapshot().tabs.find((candidate) => candidate.tabId === tabId);
			return (
				tab !== undefined &&
				tab.maximizedPanelId === null &&
				resolvePanelResize(tab.root, panelId, adjustment, multiplier, minimumSplitPercent).kind ===
					'applied'
			);
		},
		cancelClose: () => pendingConfirmationRef.current?.(false),
		closeActiveTerminal: (panelId, source: ActionInvocationSource = 'programmatic') => {
			actionRuntime.execute(WORKBENCH_ACTION_IDS.closeActiveTerminal, undefined, {
				scope: panelActions.scope,
				source,
				target: { id: panelId, type: 'panel' },
			});
		},
		confirmClose: () => {
			pendingConfirmationRef.current?.(true);
		},
		createTerminalTab: (source: ActionInvocationSource = 'programmatic') => {
			actionRuntime.execute(
				WORKBENCH_ACTION_IDS.createTerminal,
				{ groupId: null },
				{ scope: panelActions.scope, source },
			);
		},
		displayedRoot,
		focusPanel: (panelId, source: ActionInvocationSource = 'programmatic') => {
			executePanelAction(PANEL_ACTION_IDS.focus, { panelId } satisfies PanelTargetInput, source);
		},
		handlePanelVisualSelectionKeyDown,
		handlePanelVisualSelectionPointerDown,
		maximizedPanelId: activeTab?.maximizedPanelId ?? null,
		messages,
		panelCount,
		panelDrag,
		panelVisualSelection,
		panelVisualSelectionLabel,
		pendingClose,
		pendingResize,
		releasePanelDragHold,
		requestClose: (panelId, source: ActionInvocationSource = 'programmatic') => {
			const tabId = store.getActiveTabId();
			if (tabId === null) {
				return;
			}
			executePanelAction(
				PANEL_ACTION_IDS.close,
				{ panelId, tabId } satisfies PanelCloseInput,
				source,
			);
		},
		resizePanel: (panelId, adjustment, multiplier, source: ActionInvocationSource = 'keyboard') => {
			if (
				pendingResize === null ||
				pendingResize.panelId !== panelId ||
				store.getActiveTabId() !== pendingResize.tabId
			) {
				return Promise.resolve(false);
			}
			const execution = executePanelAction(
				PANEL_ACTION_IDS.resize,
				{ adjustment, multiplier, panelId } satisfies PanelResizeInput,
				source,
			);
			return execution.completion.then((result) => result.kind === 'completed');
		},
		sharedSplitResize,
		showPrimaryTerminal: (panelId, source: ActionInvocationSource = 'programmatic') => {
			executePanelAction(
				PANEL_ACTION_IDS.showPrimaryTerminal,
				{ panelId } satisfies PanelTargetInput,
				source,
			);
		},
		shortcutLabelForAction,
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
		workspaceReference,
	};
}
