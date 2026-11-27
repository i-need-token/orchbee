import { createStore, type StoreApi } from 'zustand/vanilla';

import {
	createFileOverlayState,
	transitionFileOverlay,
	type FileOverlayEffect,
	type FileOverlayEvent,
	type FileOverlayState,
} from '../model';

/** One Workspace's overlay: its state to select from and the one way to change it. */
interface FileOverlayStore {
	/** Applies an event and returns the effects for the caller to run; outcomes return as events. */
	readonly dispatch: (event: FileOverlayEvent) => readonly FileOverlayEffect[];
	readonly state: Pick<StoreApi<FileOverlayState>, 'getInitialState' | 'getState' | 'subscribe'>;
}

/** What the terminal area shows behind a Workspace's overlay. */
interface FileOverlayTerminalView {
	/** The Workspace's active Tab, or null while it shows none. */
	readonly tabId: string | null;
	/** The terminal context the Tab belongs to, or null while the Workspace's is not loaded. */
	readonly terminalContextId: string | null;
	readonly workspaceId: string;
}

const NO_EFFECTS: readonly FileOverlayEffect[] = [];

function createFileOverlayStore(initial: FileOverlayState): FileOverlayStore {
	const state = createStore<FileOverlayState>()(() => initial);
	return {
		dispatch: (event) => {
			const transition = transitionFileOverlay(state.getState(), event);
			state.setState(transition.state, true);
			return transition.effects;
		},
		state,
	};
}

/**
 * The overlays of one Workbench's Workspaces. Each starts closed when something first asks for it and
 * lives while its Workspace does, so a Workspace switch keeps every overlay's tabs, mode and tier.
 */
export class FileOverlayRegistry {
	private leftExpandOffered: boolean;
	private shown: FileOverlayTerminalView | null = null;
	private readonly stores = new Map<string, FileOverlayStore>();

	public constructor(leftExpandOffered: boolean) {
		this.leftExpandOffered = leftExpandOffered;
	}

	public store(workspaceId: string): FileOverlayStore {
		let store = this.stores.get(workspaceId);
		if (store === undefined) {
			store = createFileOverlayStore(createFileOverlayState(this.leftExpandOffered));
			this.stores.set(workspaceId, store);
		}
		return store;
	}

	/** Delivers an event to a Workspace's overlay; one that was never asked for has nothing to change. */
	public notify(workspaceId: string, event: FileOverlayEvent): readonly FileOverlayEffect[] {
		return this.stores.get(workspaceId)?.dispatch(event) ?? NO_EFFECTS;
	}

	/**
	 * Records what the terminal area shows, null outside any Workspace. Showing another Workspace or
	 * another Tab replaces the terminal behind the overlay that was in front, which shrinks to
	 * picture-in-picture. A terminal context that arrives or reloads for the same Workspace replaces
	 * nothing.
	 */
	public showTerminal(view: FileOverlayTerminalView | null): void {
		const before = this.shown;
		this.shown = view;
		if (before === null) return;
		const sameTerminal =
			view !== null &&
			view.workspaceId === before.workspaceId &&
			(view.terminalContextId !== before.terminalContextId || view.tabId === before.tabId);
		if (!sameTerminal) this.notify(before.workspaceId, { type: 'terminal-replaced' });
	}

	/** Tells every overlay whether the window offers left-expand, and starts later ones knowing it. */
	public setLeftExpandOffered(offered: boolean): void {
		if (offered === this.leftExpandOffered) return;
		this.leftExpandOffered = offered;
		for (const store of this.stores.values()) {
			store.dispatch({ leftExpandOffered: offered, type: 'layout-changed' });
		}
	}

	/**
	 * Drops the overlays of Workspaces that are gone. One holding unsaved tabs stays in memory, so the
	 * unsaved work of a Workspace that disappears waits for the person to save or discard it.
	 */
	public retain(workspaceIds: ReadonlySet<string>): void {
		for (const [workspaceId, store] of this.stores) {
			if (workspaceIds.has(workspaceId)) continue;
			if (store.state.getState().tabs.tabs.some((tab) => tab.unsaved)) continue;
			this.stores.delete(workspaceId);
		}
	}
}
