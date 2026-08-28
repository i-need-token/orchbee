import type {
	TerminalPresentationCloseActivity,
	TerminalPresentationId,
	TerminalPresentationLifecycle,
} from '../../../../terminal-presentation';

export type PanelSplitDirection = 'above' | 'below' | 'left' | 'right';
export type PanelSplitOrientation = 'horizontal' | 'vertical';

export interface TerminalPresentation {
	readonly closeActivity: TerminalPresentationCloseActivity;
	readonly currentCwd: string | null;
	readonly launchCwd: string | null;
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
