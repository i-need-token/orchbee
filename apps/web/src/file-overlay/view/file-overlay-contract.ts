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

/** The Actions the overlay's controls run; each runs as a press of its control. */
export interface FileOverlayCommands {
	/** Expand, or Restore while the tier is full: the Action that `Mod+K Mod+M` runs. */
	readonly toggleFull: () => void;
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
