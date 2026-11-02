import type { ActionInvocationSource, ActionRuntime, ActionScope } from '@orchbee/actions';

import type { WorkbenchTerminalBellEffect } from '../../../model/WorkbenchTerminalBellEffectCoordinator';
import type {
	WorkbenchTerminalBroadcastInput,
	WorkbenchTerminalBroadcastInputSink,
} from '../../../types/workbench-terminal-surface';
import type {
	PanelLayoutNode,
	PanelSplitDirection,
	PanelWorkspaceState,
	TabPanelWorkspace,
} from '../model/panel-workspace-state';
import type { PanelResizeAdjustment, PanelResizeMultiplier } from '../model/resolvePanelResize';
import type { PanelVisualSelectionSession } from '../model/panel-visual-selection';
import type { PanelBroadcastSessionState } from '../model/panel-broadcast-input';
import type {
	KeyboardEvent as ReactKeyboardEvent,
	PointerEvent as ReactPointerEvent,
	RefObject,
} from 'react';
import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

export interface PanelDropTarget {
	readonly direction: PanelSplitDirection;
	readonly panelId: string;
}

export interface PanelDragGhost {
	readonly blockStart: number;
	readonly height: number;
	readonly inlineStart: number;
	readonly mountSnapshot: (host: HTMLDivElement | null) => void;
	readonly scale: number;
	readonly sourceHeight: number;
	readonly sourceWidth: number;
	readonly width: number;
}

export interface PanelWorkspaceMessages {
	readonly broadcastDeliveryFailed: string;
	readonly broadcastDismiss: string;
	readonly broadcastEnd: string;
	readonly broadcastHideInput: string;
	readonly broadcastInputLabel: string;
	readonly broadcastInputPlaceholder: string;
	readonly broadcastPartialDelivery: (delivered: number, total: number) => string;
	readonly broadcastShowInput: string;
	readonly broadcastStart: string;
	readonly broadcastTargetCount: (count: number) => string;
	readonly broadcastTargetInvalidated: string;
	readonly cancel: string;
	readonly childPresentationsLabel: string;
	readonly close: string;
	readonly closeAnyway: string;
	readonly closeRunningDescription: (count: number) => string;
	readonly closeRunningTitle: string;
	readonly equalize: string;
	readonly maximize: string;
	readonly minimize: string;
	readonly more: string;
	readonly newChildTerminal: string;
	readonly newTab: string;
	readonly newTabCurrentDirectory: string;
	readonly returnToPrimary: string;
	readonly resize: string;
	readonly resizeDescription: string;
	readonly resizeDone: string;
	readonly resizeNarrower: string;
	readonly resizeReset: string;
	readonly resizeShorter: string;
	readonly resizeTaller: string;
	readonly resizeTitle: string;
	readonly resizeUnavailable: string;
	readonly resizeWider: string;
	readonly split: string;
	readonly splitAbove: string;
	readonly splitBelow: string;
	readonly splitLeft: string;
	readonly splitRight: string;
	readonly visualSelectionFocusLabel: string;
	readonly visualSelectionFocusTarget: (label: string) => string;
	readonly visualSelectionSwapLabel: string;
	readonly visualSelectionSwapTarget: (label: string) => string;
}

export interface PendingPanelClose {
	readonly panelId: string;
	readonly runningCount: number;
}

export interface PendingPanelResize {
	readonly panelId: string;
	readonly tabId: string;
}

/** A divider whose position every Client shares: nothing leaves this Client until it is released. */
export interface SharedSplitResize {
	/** Settles once this Client's layout holds what happened. */
	readonly commit: (splitId: string, leadingWeight: number) => Promise<void>;
	/** The smallest share either side can hold, out of 100. */
	readonly minimumChildPercent: number;
}

export interface WorkbenchPanelWorkspaceView {
	readonly onTerminalEdit?: ((presentationId: string) => void) | undefined;
	readonly actionRuntime: ActionRuntime;
	readonly actionScope: ActionScope;
	readonly admitTerminalBellEffect: (event: WorkbenchTerminalBellEffect) => boolean;
	readonly activateChildTerminal: (
		panelId: string,
		childPresentationId: TerminalPresentationId,
		source?: ActionInvocationSource,
	) => void;
	readonly activeTab: TabPanelWorkspace | null;
	readonly bindBroadcastInputSink: (
		presentationId: TerminalPresentationId,
		sink: WorkbenchTerminalBroadcastInputSink,
	) => { dispose(): void };
	readonly broadcastInput: PanelBroadcastSessionState;
	readonly canStartBroadcastInput: (panelId: string) => boolean;
	readonly cancelPanelVisualSelection: (restoreSourceFocus: boolean) => void;
	readonly cancelPanelResize: () => void;
	readonly canResizePanel: (
		panelId: string,
		adjustment: PanelResizeAdjustment,
		multiplier?: PanelResizeMultiplier,
	) => boolean;
	readonly canCreateTerminalTabInCurrentDirectory: (panelId: string) => boolean;
	readonly cancelClose: () => void;
	readonly closeActiveTerminal: (panelId: string, source?: ActionInvocationSource) => void;
	readonly confirmClose: () => void;
	readonly createChildTerminal: (panelId: string, source?: ActionInvocationSource) => void;
	readonly createTerminalTab: (source?: ActionInvocationSource) => void;
	readonly createTerminalTabInCurrentDirectory: (
		panelId: string,
		source?: ActionInvocationSource,
	) => void;
	readonly displayedRoot: PanelLayoutNode | null;
	readonly equalizePanels: (panelId: string, source?: ActionInvocationSource) => void;
	readonly enqueueBroadcastInput: (input: WorkbenchTerminalBroadcastInput) => void;
	readonly endBroadcastInput: () => void;
	readonly focusPanel: (panelId: string, source?: ActionInvocationSource) => void;
	readonly handlePanelVisualSelectionKeyDown: (event: ReactKeyboardEvent<HTMLDivElement>) => void;
	readonly handlePanelVisualSelectionPointerDown: (
		event: ReactPointerEvent<HTMLDivElement>,
	) => void;
	readonly maximizedPanelId: string | null;
	readonly messages: PanelWorkspaceMessages;
	readonly panelCount: number;
	readonly panelDrag: {
		readonly draggingPanelId: string | null;
		readonly dropTarget: PanelDropTarget | null;
		/** Null once released: the drop can stay on screen after the pointer lets go. */
		readonly ghost: PanelDragGhost | null;
		readonly previewRoot: PanelLayoutNode | null;
		/** The presentation revision this drag belongs to. Only a layer showing it shows the drop. */
		readonly sourceRevision: string | null;
	};
	readonly panelVisualSelection: PanelVisualSelectionSession | null;
	readonly panelVisualSelectionLabel: string | undefined;
	readonly pendingClose: PendingPanelClose | null;
	readonly pendingResize: PendingPanelResize | null;
	/** Stops showing a released drop once no layer of its revision remains. */
	readonly releasePanelDragHold: (sourceRevision: string) => void;
	readonly requestClose: (panelId: string, source?: ActionInvocationSource) => void;
	readonly dismissBroadcastInput: () => void;
	readonly resizeAdjacentPanels: (
		splitId: string,
		separatorIndex: number,
		leadingWeight: number,
	) => void;
	/** Settles true when the size changed. */
	readonly resizePanel: (
		panelId: string,
		adjustment: PanelResizeAdjustment,
		multiplier: PanelResizeMultiplier,
		source?: ActionInvocationSource,
	) => Promise<boolean>;
	readonly sharedSplitResize?: SharedSplitResize | undefined;
	readonly showPrimaryTerminal: (panelId: string, source?: ActionInvocationSource) => void;
	readonly shortcutLabelForAction: (actionId: string) => string | undefined;
	readonly splitPanel: (
		panelId: string,
		direction: PanelSplitDirection,
		source?: ActionInvocationSource,
	) => string | null;
	readonly startPanelDrag: (panelId: string, event: ReactPointerEvent<HTMLElement>) => void;
	readonly startBroadcastInput: (panelId: string, source?: ActionInvocationSource) => void;
	readonly startPanelResize: (panelId: string, source?: ActionInvocationSource) => void;
	readonly state: PanelWorkspaceState;
	readonly togglePanelMaximization: (panelId: string, source?: ActionInvocationSource) => void;
	readonly toggleBroadcastInputVisibility: () => void;
	readonly workspaceReference: RefObject<HTMLDivElement | null>;
}
