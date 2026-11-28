import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import type { ProductIconName } from '@orchbee/ui/components';
import type { ComponentType, Ref, RefObject } from 'react';

import type { PointerDropTarget } from '../../pointer-drag';
import type { FileOverlayEvent, FileOverlayPipPosition } from '../model';
import type { FileOverlayStore, FileTabContents } from '../workspace-stores';

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
	 * Gives the overlay the keys once it shows the change just dispatched: the active tab while the
	 * overlay is open, as VS Code focuses the editor it opens, or picture-in-picture while it stays
	 * shrunk. Nothing happens while the overlay is closed.
	 */
	focus(): void;
}

/** The Actions the overlay's controls run; each runs as a press of its control. */
export interface FileOverlayCommands {
	/** Closes the overlay with every tab in it: picture-in-picture's close button. */
	readonly closeOverlay: () => void;
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

/** What the text editor in the overlay needs of the Workbench around it. */
export interface FileOverlayEditorHost {
	/** The runtime where the editor's own commands join the Workbench's Actions. */
	readonly actionRuntime: ActionRuntime;
	/** The Workbench's scope, under which the editor's commands resolve. */
	readonly actionScope: ActionScope;
	/**
	 * Whether `target` lies in one of the Workbench's focus regions, which takes the keys from the
	 * editor. Focus outside them, as in Action search or a menu, leaves the editor its commands.
	 */
	readonly inFocusRegion: (target: EventTarget) => boolean;
	/** Opens Action search on Actions: the editor menu's last item, where VS Code's is Command Palette. */
	readonly searchActions: () => void;
	/** The keys that open Action search on Actions, absent while a configured command holds them. */
	readonly searchActionsShortcut: string | undefined;
}

/** The keys bound to the overlay's Actions, each absent while nothing binds it. */
export interface FileOverlayShortcutLabels {
	readonly closeOverlay: string | undefined;
	readonly toggleFull: string | undefined;
}

/** What the Workbench tells the shown Workspace's overlay. */
export interface FileOverlayHostProps {
	readonly commands: FileOverlayCommands;
	/** The content behind the shown Workspace's tabs; null where no file session serves it. */
	readonly contents: FileTabContents | null;
	/** Applies an event to the shown overlay and runs the effects it returns. */
	readonly dispatch: (event: FileOverlayEvent) => void;
	readonly editor: FileOverlayEditorHost;
	/**
	 * The File Bar's side of the terminal area. Moving it shifts the terminal area without resizing
	 * it, so the overlay measures what it lies over again.
	 */
	readonly fileBarPosition: 'left' | 'right';
	/** Gives the keys to the current terminal, as Esc in picture-in-picture does. */
	readonly focusTerminal: () => void;
	/** The platform whose pointer and key conventions the overlay follows. */
	readonly platform: KeyboardShortcutPlatform;
	/** Keeps where a drag left picture-in-picture, which every Workspace's overlay shares. */
	readonly movePip: (position: FileOverlayPipPosition) => void;
	/** Where picture-in-picture shows; null places it at the terminal area's bottom-right corner. */
	readonly pipPosition: FileOverlayPipPosition | null;
	/** Receives how the Workbench gives the keys to the overlay. */
	readonly ref?: Ref<FileOverlayFocus> | undefined;
	readonly regions: FileOverlayRegionElements;
	/**
	 * Gives the keys back as the overlay closes with them: to the control that last brought it up, if
	 * that control still shows, else to the current terminal.
	 */
	readonly returnFocus: () => void;
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
