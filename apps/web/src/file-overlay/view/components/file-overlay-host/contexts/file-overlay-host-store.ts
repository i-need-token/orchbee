import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { createContext, createElement, useContext, useMemo, type ReactNode } from 'react';
import { useStore } from 'zustand';

import type { FileOverlayEvent, FileOverlayState } from '../../../../model';
import type { FileOverlayStore, FileTabContents } from '../../../../workspace-stores';
import type {
	FileOverlayCommands,
	FileOverlayEditorHost,
	FileOverlayShortcutLabels,
} from '../../../file-overlay-contract';
import type { FileOverlayFocusRequest } from '../hooks/useFileOverlayFocusRequest';

interface FileOverlayHostValue {
	readonly commands: FileOverlayCommands;
	readonly contents: FileTabContents | null;
	readonly dispatch: (event: FileOverlayEvent) => void;
	readonly editor: FileOverlayEditorHost;
	readonly focusRequest: FileOverlayFocusRequest;
	readonly focusTerminal: () => void;
	readonly platform: KeyboardShortcutPlatform;
	readonly returnFocus: () => void;
	readonly shortcutLabels: FileOverlayShortcutLabels;
	readonly store: FileOverlayStore;
}

const FileOverlayHostContext = createContext<FileOverlayHostValue | null>(null);

interface FileOverlayHostStoreProviderProps extends FileOverlayHostValue {
	readonly children: ReactNode;
}

/**
 * Gives the overlay's components the shown Workspace's overlay, the one way to change it, the
 * Actions its controls run, and what they need of the Workbench around them.
 */
export function FileOverlayHostStoreProvider({
	children,
	commands,
	contents,
	dispatch,
	editor,
	focusRequest,
	focusTerminal,
	platform,
	returnFocus,
	shortcutLabels,
	store,
}: FileOverlayHostStoreProviderProps): ReactNode {
	const value = useMemo(
		() => ({
			commands,
			contents,
			dispatch,
			editor,
			focusRequest,
			focusTerminal,
			platform,
			returnFocus,
			shortcutLabels,
			store,
		}),
		[
			commands,
			contents,
			dispatch,
			editor,
			focusRequest,
			focusTerminal,
			platform,
			returnFocus,
			shortcutLabels,
			store,
		],
	);
	return createElement(FileOverlayHostContext.Provider, { value }, children);
}

function useFileOverlayHostValue(): FileOverlayHostValue {
	const value = useContext(FileOverlayHostContext);
	if (value === null) throw new Error('The file overlay is read outside its store provider.');
	return value;
}

/** Selects what a component renders from the shown overlay's state. */
export function useFileOverlayHostStore<Selected>(
	selector: (state: FileOverlayState) => Selected,
): Selected {
	return useStore(useFileOverlayHostValue().store.state, selector);
}

/**
 * Reads the shown overlay's state as it is now, for a handler that runs after the render that set it
 * up, as when the overlay goes away around it.
 */
export function useFileOverlayHostReadState(): () => FileOverlayState {
	return useFileOverlayHostValue().store.state.getState;
}

/** Applies an event to the shown overlay and runs the effects it returns. */
export function useFileOverlayHostDispatch(): (event: FileOverlayEvent) => void {
	return useFileOverlayHostValue().dispatch;
}

/** The content behind the shown Workspace's tabs; null where no file session serves it. */
export function useFileOverlayHostContents(): FileTabContents | null {
	return useFileOverlayHostValue().contents;
}

/** What the text editor needs of the Workbench around the overlay. */
export function useFileOverlayHostEditor(): FileOverlayEditorHost {
	return useFileOverlayHostValue().editor;
}

/** The Actions the overlay's controls run, each as a press of that control. */
export function useFileOverlayHostCommands(): FileOverlayCommands {
	return useFileOverlayHostValue().commands;
}

/** The Workbench's asks to give the active tab the keys. */
export function useFileOverlayHostFocusRequest(): FileOverlayFocusRequest {
	return useFileOverlayHostValue().focusRequest;
}

/** Gives the keys to the current terminal. */
export function useFileOverlayHostFocusTerminal(): () => void {
	return useFileOverlayHostValue().focusTerminal;
}

/** The platform whose pointer and key conventions the overlay follows. */
export function useFileOverlayHostPlatform(): KeyboardShortcutPlatform {
	return useFileOverlayHostValue().platform;
}

/**
 * Gives the keys back as the overlay closes with them: to the control that last brought it up while
 * that control shows, else to the current terminal.
 */
export function useFileOverlayHostReturnFocus(): () => void {
	return useFileOverlayHostValue().returnFocus;
}

/** The keys bound to the overlay's Actions, for the controls' tooltips. */
export function useFileOverlayHostShortcutLabels(): FileOverlayShortcutLabels {
	return useFileOverlayHostValue().shortcutLabels;
}
