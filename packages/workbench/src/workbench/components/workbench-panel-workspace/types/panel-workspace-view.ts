import type { ActionInvocationSource } from '@orchbee/actions';
import type {
	PanelLayoutNode,
	PanelSplitDirection,
	PanelWorkspaceState,
	TabPanelWorkspace,
} from '../model/panel-workspace-state';
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
	readonly childTerminalsLabel: string;
	readonly close: string;
	readonly closeAnyway: string;
	readonly closeRunningDescription: (count: number) => string;
	readonly closeRunningTitle: string;
	readonly maximize: string;
	readonly minimize: string;
	readonly more: string;
	readonly newChildTerminal: string;
	readonly returnToPrimary: string;
	readonly splitAbove: string;
	readonly splitBelow: string;
	readonly splitLeft: string;
	readonly splitRight: string;
}

export interface PendingPanelClose {
	readonly panelId: string;
	readonly runningCount: number;
}

export interface WorkbenchPanelWorkspaceView {
	readonly activateChildTerminal: (
		panelId: string,
		childTerminalId: string,
		source?: ActionInvocationSource,
	) => void;
	readonly activeTab: TabPanelWorkspace | null;
	readonly cancelClose: () => void;
	readonly confirmClose: () => void;
	readonly createChildTerminal: (panelId: string, source?: ActionInvocationSource) => void;
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
	readonly requestClose: (panelId: string, source?: ActionInvocationSource) => void;
	readonly resizeAdjacentPanels: (
		splitId: string,
		separatorIndex: number,
		leadingWeight: number,
	) => void;
	readonly showPrimaryTerminal: (panelId: string, source?: ActionInvocationSource) => void;
	readonly splitPanel: (
		panelId: string,
		direction: PanelSplitDirection,
		source?: ActionInvocationSource,
	) => string | null;
	readonly startPanelDrag: (panelId: string, event: ReactPointerEvent<HTMLElement>) => void;
	readonly state: PanelWorkspaceState;
	readonly togglePanelMaximization: (panelId: string, source?: ActionInvocationSource) => void;
}
