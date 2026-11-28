import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import type { ComponentType, RefObject } from 'react';

import type { FileOverlayEvent } from '../model';
import type { FileOverlayStore } from '../workspace-stores';

/** The Workbench's elements whose boxes place the overlay. */
export interface FileOverlayRegionElements {
	/** The Workbench root, whose top-left corner the boxes are measured from. */
	readonly root: RefObject<HTMLElement | null>;
	/** The Sidebar's panel, read only while the Sidebar shows docked. */
	readonly sidebar: RefObject<HTMLElement | null>;
	/** The terminal area, from the 36px row down to the window's bottom. */
	readonly terminalArea: RefObject<HTMLElement | null>;
}

/** What invoked one of the overlay's controls, as its Action reports the outcome. */
export type FileOverlayCommandSource = 'button' | 'keyboard' | 'pointer';

/** The Actions the overlay's controls run; each runs as a press of its control. */
export interface FileOverlayCommands {
	/** Close Editor for one tab: its close button or a middle click. */
	readonly closeTab: (tabId: string, source: FileOverlayCommandSource) => void;
	/** Keep Editor for the preview tab, which a double-click runs. */
	readonly keepTab: (tabId: string, source: FileOverlayCommandSource) => void;
	/** Shows one tab: a press on it, or a key that moves along the row. */
	readonly selectTab: (tabId: string, source: FileOverlayCommandSource) => void;
	/** Expand, or Restore while the tier is full: the Action that `Mod+K Mod+M` runs. */
	readonly toggleFull: () => void;
	/** Unpin Editor for one sticky tab: its pin button. */
	readonly unpinTab: (tabId: string, source: FileOverlayCommandSource) => void;
}

/** The keys bound to the overlay's Actions, each absent while nothing binds it. */
export interface FileOverlayShortcutLabels {
	readonly toggleFull: string | undefined;
}

/** What the Workbench tells the shown Workspace's overlay. */
export interface FileOverlayHostProps {
	readonly commands: FileOverlayCommands;
	/** Applies an event to the shown overlay and runs the effects it returns. */
	readonly dispatch: (event: FileOverlayEvent) => void;
	/** Gives the keys to the current terminal, as when the overlay leaves with the focus inside. */
	readonly focusTerminal: () => void;
	/** The platform whose pointer and key conventions the overlay follows. */
	readonly platform: KeyboardShortcutPlatform;
	readonly regions: FileOverlayRegionElements;
	readonly shortcutLabels: FileOverlayShortcutLabels;
	/** Whether the Sidebar shows docked, which left-expand covers. */
	readonly sidebarDocked: boolean;
	/** The shown Workspace's overlay. */
	readonly store: FileOverlayStore;
}

/** What the overlay's chunk gives the Workbench, which decides whose overlay shows. */
export interface FileOverlayModule {
	/** The shown Workspace's overlay, placed in the Workbench root over its layout. */
	readonly FileOverlayHost: ComponentType<FileOverlayHostProps>;
}
