import type {
	TerminalPresentationId,
	TerminalPresentationLifecycle,
} from '@orchbee/workbench-client-protocol';

export type PanelSplitDirection = 'above' | 'below' | 'left' | 'right';
export type PanelSplitOrientation = 'horizontal' | 'vertical';

export interface TerminalPresentation {
	/**
	 * The label the Server gave the terminal when it was created. No person chose it, so only an
	 * Agent row shows it, as the TUI's does, and never an automatic number.
	 */
	readonly creationLabel?: string | undefined;
	readonly currentCwd: string | null;
	/** A name a person chose, which outranks everything the terminal reports about itself. */
	readonly customTitle?: string | undefined;
	readonly lifecycle: TerminalPresentationLifecycle;
	/**
	 * What the natural title names for an Agent row: null when nothing is left or a console's launch
	 * path names only its program; absent until this window reads the terminal's title.
	 */
	readonly naturalTask?: string | null | undefined;
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
