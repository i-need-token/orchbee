import type {
	TerminalPresentationCloseActivity,
	TerminalPresentationId,
	TerminalPresentationLifecycle,
} from '@orchbee/workbench-client-protocol';

export type PanelSplitDirection = 'above' | 'below' | 'left' | 'right';
export type PanelSplitOrientation = 'horizontal' | 'vertical';

export interface TerminalPresentation {
	readonly closeActivity: TerminalPresentationCloseActivity;
	readonly currentCwd: string | null;
	/** A name a person chose, which outranks everything the terminal reports about itself. */
	readonly customTitle?: string | undefined;
	readonly lifecycle: TerminalPresentationLifecycle;
	readonly naturalTitle: string | null;
	readonly presentationId: TerminalPresentationId;
	readonly title: string;
}

export interface PanelLeaf {
	readonly activeChildPresentationId: TerminalPresentationId | null;
	readonly childPresentations: readonly TerminalPresentation[];
	readonly id: string;
	readonly kind: 'panel';
	readonly primaryPresentation: TerminalPresentation;
}

interface PanelSplitChild {
	readonly id: string;
	readonly node: PanelLayoutNode;
	readonly weight: number;
}

export interface PanelSplit {
	readonly children: readonly PanelSplitChild[];
	readonly id: string;
	readonly kind: 'split';
	readonly orientation: PanelSplitOrientation;
}

export type PanelLayoutNode = PanelLeaf | PanelSplit;
