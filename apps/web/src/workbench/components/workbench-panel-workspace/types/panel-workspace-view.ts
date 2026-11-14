import type { ActionInvocationSource, ActionRuntime, ActionScope } from '@orchbee/actions';

import type { TerminalCloseImpact } from '../../../model/terminal-close-impact';
import type { WorkbenchAgentStatusView } from '../../../utils/formatWorkbenchAgentStatus';
import type { WorkbenchTerminalBellEffect } from '../../../model/WorkbenchTerminalBellEffectCoordinator';
import type {
	PanelLayoutNode,
	PanelSplitDirection,
	PanelWorkspaceState,
	TabPanelWorkspace,
} from '../model/panel-workspace-state';
import type { PanelVisualSelectionSession } from '../model/panel-visual-selection';
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
	readonly cancel: string;
	readonly rename: string;
	readonly childPresentationsLabel: string;
	readonly close: string;
	readonly closeLastPanel: string;
	readonly closeTab: (tab: string) => string;
	readonly closeTitle: string;
	readonly closeWarning: (impact: TerminalCloseImpact) => string;
	readonly maximize: string;
	readonly more: string;
	readonly restore: string;
	readonly returnToPrimary: string;
	readonly splitBelow: string;
	readonly splitRight: string;
	readonly visualSelectionFocusLabel: string;
	readonly visualSelectionFocusTarget: (label: string) => string;
	readonly visualSelectionSwapLabel: string;
	readonly visualSelectionSwapTarget: (label: string) => string;
}

/** A close question as the TUI composes it: the terminal it shows, its Tab, and what it ends. */
export interface PendingPanelClose {
	/** The Panel is its Tab's last, so the Tab closes with it. */
	readonly dissolvesTab: boolean;
	readonly impact: TerminalCloseImpact;
	readonly panelId: string;
	readonly tabTitle: string;
	/** The name the Panel's header shows. */
	readonly target: string;
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
	readonly cancelPanelVisualSelection: (restoreSourceFocus: boolean) => void;
	readonly cancelClose: () => void;
	readonly closeActiveTerminal: (panelId: string, source?: ActionInvocationSource) => void;
	readonly renamePanel: (panelId: string, source?: ActionInvocationSource) => void;
	readonly confirmClose: () => void;
	readonly createTerminalTab: (source?: ActionInvocationSource) => void;
	/** Starts a held restore's terminal; absent where the authority holds no restore. */
	readonly restoreHeldTerminal?:
		| ((presentationId: string, directory: 'home' | 'saved') => Promise<void>)
		| undefined;
	readonly displayedRoot: PanelLayoutNode | null;
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
	/** Each reported Agent's status view by Presentation ID; a Panel header draws its displayed terminal's. */
	readonly presentationAgentStatuses: ReadonlyMap<string, WorkbenchAgentStatusView>;
	/** Stops showing a released drop once no layer of its revision remains. */
	readonly releasePanelDragHold: (sourceRevision: string) => void;
	readonly requestClose: (panelId: string, source?: ActionInvocationSource) => void;
	readonly sharedSplitResize: SharedSplitResize;
	readonly showPrimaryTerminal: (panelId: string, source?: ActionInvocationSource) => void;
	readonly shortcutLabelForAction: (actionId: string) => string | undefined;
	readonly splitPanel: (
		panelId: string,
		direction: PanelSplitDirection,
		source?: ActionInvocationSource,
	) => string | null;
	readonly startPanelDrag: (panelId: string, event: ReactPointerEvent<HTMLElement>) => void;
	readonly state: PanelWorkspaceState;
	readonly togglePanelMaximization: (panelId: string, source?: ActionInvocationSource) => void;
	readonly workspaceReference: RefObject<HTMLDivElement | null>;
}
