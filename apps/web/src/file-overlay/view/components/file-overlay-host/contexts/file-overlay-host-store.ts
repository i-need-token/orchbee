import { createContext, createElement, useContext, useMemo, type ReactNode } from 'react';
import { useStore } from 'zustand';

import type { FileOverlayEvent, FileOverlayState } from '../../../../model';
import type { FileOverlayStore } from '../../../../workspace-stores';
import type {
	FileOverlayCommands,
	FileOverlayShortcutLabels,
} from '../../../file-overlay-contract';

interface FileOverlayHostValue {
	readonly commands: FileOverlayCommands;
	readonly dispatch: (event: FileOverlayEvent) => void;
	readonly shortcutLabels: FileOverlayShortcutLabels;
	readonly store: FileOverlayStore;
}

const FileOverlayHostContext = createContext<FileOverlayHostValue | null>(null);

interface FileOverlayHostStoreProviderProps extends FileOverlayHostValue {
	readonly children: ReactNode;
}

/**
 * Gives the overlay's components the shown Workspace's overlay, the one way to change it, and the
 * Actions its controls run.
 */
export function FileOverlayHostStoreProvider({
	children,
	commands,
	dispatch,
	shortcutLabels,
	store,
}: FileOverlayHostStoreProviderProps): ReactNode {
	const value = useMemo(
		() => ({ commands, dispatch, shortcutLabels, store }),
		[commands, dispatch, shortcutLabels, store],
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

/** Applies an event to the shown overlay and runs the effects it returns. */
export function useFileOverlayHostDispatch(): (event: FileOverlayEvent) => void {
	return useFileOverlayHostValue().dispatch;
}

/** The Actions the overlay's controls run, each as a press of that control. */
export function useFileOverlayHostCommands(): FileOverlayCommands {
	return useFileOverlayHostValue().commands;
}

/** The keys bound to the overlay's Actions, for the controls' tooltips. */
export function useFileOverlayHostShortcutLabels(): FileOverlayShortcutLabels {
	return useFileOverlayHostValue().shortcutLabels;
}
