import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import type { ProductIconName } from '@orchbee/ui/components';
import type { ComponentType, Ref, RefObject } from 'react';

import type { PointerDropTarget } from '../../pointer-drag';
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

/** An item of one of the overlay's menus. */
export interface FileOverlayMenuCommand {
	readonly disabled: boolean;
	readonly id: string;
	readonly kind: 'command';
	readonly label: string;
	/** Runs the item's Action as chosen from the menu. */
	readonly run: () => void;
	/** The keys that run the item's Action now, absent while nothing binds it. */
	readonly shortcut: string | undefined;
}

/** One row of one of the overlay's menus: an item, or the line between two of VS Code's groups. */
export type FileOverlayMenuEntry =
	| FileOverlayMenuCommand
	| { readonly id: string; readonly kind: 'separator' };

/** A title action that VS Code shows as a button, one of its `navigation` group. */
export interface FileOverlayTitleButton extends FileOverlayMenuCommand {
	readonly icon: ProductIconName;
}

/**
 * The active tab's title actions, which VS Code shows at the end of the tab row: its `navigation`
 * group as buttons, then More Actions with its other groups.
 */
export interface FileOverlayTitleActions {
	readonly buttons: readonly FileOverlayTitleButton[];
	/** More Actions' items; empty, it leaves More Actions out. */
	readonly more: readonly FileOverlayMenuEntry[];
}

/** How the Workbench gives the keys to the shown overlay. */
export interface FileOverlayFocus {
	/**
	 * Gives the active tab the keys once the overlay shows the change just dispatched, as VS Code
	 * focuses the editor it opens; nothing happens while the overlay does not open.
	 */
	focus(): void;
}

/** The Actions the overlay's controls run; each runs as a press of its control. */
export interface FileOverlayCommands {
	/** Close Editor for one tab: its close button or a middle click. */
	readonly closeTab: (tabId: string, source: FileOverlayCommandSource) => void;
	/** Keep Editor for the preview tab, which a double-click runs. */
	readonly keepTab: (tabId: string, source: FileOverlayCommandSource) => void;
	/** Moves one tab to a place in the row, counted from 0: the release of a drag along the row. */
	readonly moveTab: (tabId: string, index: number, source: FileOverlayCommandSource) => void;
	/** Shows one tab: a press on it, or a key that moves along the row. */
	readonly selectTab: (tabId: string, source: FileOverlayCommandSource) => void;
	/** One tab's menu as it opens: the items VS Code's tab menu shows for that tab, in its groups. */
	readonly tabMenu: (tabId: string) => readonly FileOverlayMenuEntry[];
	/** The title actions VS Code shows while a tab is active. */
	readonly titleActions: (tabId: string) => FileOverlayTitleActions;
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
	/** Receives how the Workbench gives the keys to the overlay. */
	readonly ref?: Ref<FileOverlayFocus> | undefined;
	readonly regions: FileOverlayRegionElements;
	/**
	 * Takes drops on the scrim, which covers the current terminal, as a drop on that terminal; null
	 * where nothing drops there.
	 */
	readonly scrimDrop: PointerDropTarget | null;
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
