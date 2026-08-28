import type { ActionInvocationSource, ActionRuntime, ActionScope } from '@orchbee/actions';

import type { TerminalPresentationId } from '../../../../terminal-presentation';
import type { WorkbenchTerminalBellEffect } from '../../../model/WorkbenchTerminalBellEffectCoordinator';
import type {
	PanelLayoutNode,
	PanelSplitDirection,
	PanelWorkspaceState,
	TabPanelWorkspace,
} from '../model/panel-workspace-state';
import type { PanelResizeAdjustment, PanelResizeMultiplier } from '../model/resolvePanelResize';
import type { PointerEvent as ReactPointerEvent } from 'react';

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
	readonly cancel: string;
	readonly childPresentationsLabel: string;
	readonly close: string;
	readonly closeAnyway: string;
	readonly closeRunningDescription: (count: number) => string;
	readonly closeRunningTitle: string;
	readonly maximize: string;
	readonly minimize: string;
	readonly more: string;
	readonly newChildTerminal: string;
	readonly newTab: string;
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
}

export interface PendingPanelClose {
	readonly panelId: string;
	readonly runningCount: number;
}

export interface PendingPanelResize {
	readonly panelId: string;
	readonly tabId: string;
}

export interface WorkbenchPanelWorkspaceView {
	readonly actionRuntime: ActionRuntime;
	readonly actionScope: ActionScope;
	readonly admitTerminalBellEffect: (event: WorkbenchTerminalBellEffect) => boolean;
	readonly activateChildTerminal: (
		panelId: string,
		childPresentationId: TerminalPresentationId,
		source?: ActionInvocationSource,
	) => void;
	readonly activeTab: TabPanelWorkspace | null;
	readonly cancelPanelResize: () => void;
	readonly canResizePanel: (
		panelId: string,
		adjustment: PanelResizeAdjustment,
		multiplier?: PanelResizeMultiplier,
	) => boolean;
	readonly cancelClose: () => void;
	readonly closeActiveTerminal: (panelId: string, source?: ActionInvocationSource) => void;
	readonly confirmClose: () => void;
	readonly createChildTerminal: (panelId: string, source?: ActionInvocationSource) => void;
	readonly createTerminalTab: (source?: ActionInvocationSource) => void;
	readonly displayedRoot: PanelLayoutNode | null;
	readonly focusPanel: (panelId: string, source?: ActionInvocationSource) => void;
	readonly maximizedPanelId: string | null;
	readonly messages: PanelWorkspaceMessages;
	readonly panelCount: number;
	readonly panelDrag: {
		readonly draggingPanelId: string | null;
		readonly dropTarget: PanelDropTarget | null;
		readonly ghost: PanelDragGhost | null;
		readonly previewRoot: PanelLayoutNode | null;
	};
	readonly pendingClose: PendingPanelClose | null;
	readonly pendingResize: PendingPanelResize | null;
	readonly requestClose: (panelId: string, source?: ActionInvocationSource) => void;
	readonly resizeAdjacentPanels: (
		splitId: string,
		separatorIndex: number,
		leadingWeight: number,
	) => void;
	readonly resizePanel: (
		panelId: string,
		adjustment: PanelResizeAdjustment,
		multiplier: PanelResizeMultiplier,
		source?: ActionInvocationSource,
	) => boolean;
	readonly showPrimaryTerminal: (panelId: string, source?: ActionInvocationSource) => void;
	readonly shortcutLabelForAction: (actionId: string) => string | undefined;
	readonly splitPanel: (
		panelId: string,
		direction: PanelSplitDirection,
		source?: ActionInvocationSource,
	) => string | null;
	readonly startPanelDrag: (panelId: string, event: ReactPointerEvent<HTMLElement>) => void;
	readonly startPanelResize: (panelId: string, source?: ActionInvocationSource) => void;
	readonly state: PanelWorkspaceState;
	readonly togglePanelMaximization: (panelId: string, source?: ActionInvocationSource) => void;
}
