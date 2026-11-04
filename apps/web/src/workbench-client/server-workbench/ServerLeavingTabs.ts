import type { WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

/** Whether the window leaves a Tab out of what it shows, as {@link ServerLeavingTabs} decides. */
export type ServerTabLeaving = (terminalContextId: string, tabId: string) => boolean;

/** Whether a Tab is the one the window shows in its terminal context. */
export type ServerTabSelected = (terminalContextId: string, tabId: string) => boolean;

/** What a close tells {@link ServerLeavingTabs} about a Tab whose close it follows. */
export interface ServerTabLeave {
	/**
	 * The Tab's close progresses, and the window follows it until it calls the returned function,
	 * once the close settles or is no longer followed. `inSequence` says that the close is one of
	 * several Tabs closing as one sequence.
	 */
	leave(terminalContextId: string, tabId: string, inSequence: boolean): () => void;
}

interface FollowedTab {
	/** How many followed closes hold the Tab out. */
	readonly followers: number;
	readonly tabId: string;
	readonly terminalContextId: string;
}

const STAY = (): void => undefined;

/**
 * The Tabs whose progressing close the window follows. As the TUI leaves out a Tab whose close is
 * progressing, the window projects the Server's snapshot without them, so a Tab leaves the strip
 * when its close progresses instead of when the publication removes it, and the selection moves
 * on as the publication reconciles. It shows again once no close of it is followed, which a
 * refused, stopped, unknown or lost close ends with; a completed close has removed it by then.
 * While several Tabs close as one sequence the selected one stays, because the TUI keeps the
 * selection during a sequence; the selection moves once the Server removes that Tab.
 */
export class ServerLeavingTabs implements ServerTabLeave {
	readonly #followed = new Map<string, FollowedTab>();
	readonly #onChange: (returned: boolean) => void;
	readonly #selected: ServerTabSelected;
	readonly #snapshot: () => WorkbenchSnapshot;

	/**
	 * `onChange` hears each Tab the snapshot holds leave or return, with whether one returned, so the
	 * snapshot can be projected again.
	 */
	public constructor(
		snapshot: () => WorkbenchSnapshot,
		selected: ServerTabSelected,
		onChange: (returned: boolean) => void,
	) {
		this.#onChange = onChange;
		this.#selected = selected;
		this.#snapshot = snapshot;
	}

	public readonly leaving: ServerTabLeaving = (terminalContextId, tabId) =>
		this.#followed.has(tabKey(terminalContextId, tabId));

	public leave(terminalContextId: string, tabId: string, inSequence: boolean): () => void {
		if (inSequence && this.#selected(terminalContextId, tabId)) return STAY;
		const key = tabKey(terminalContextId, tabId);
		const followers = this.#followed.get(key)?.followers ?? 0;
		this.#followed.set(key, { followers: followers + 1, tabId, terminalContextId });
		if (followers === 0 && holdsTab(this.#snapshot(), terminalContextId, tabId)) {
			this.#onChange(false);
		}
		let followed = true;
		return () => {
			if (!followed) return;
			followed = false;
			this.#stay(key);
		};
	}

	/** Forgets each Tab `snapshot` no longer holds, since Tab IDs never return. */
	public prune(snapshot: WorkbenchSnapshot): void {
		for (const [key, { tabId, terminalContextId }] of this.#followed) {
			if (!holdsTab(snapshot, terminalContextId, tabId)) this.#followed.delete(key);
		}
	}

	#stay(key: string): void {
		const followed = this.#followed.get(key);
		if (followed === undefined) return;
		if (followed.followers > 1) {
			this.#followed.set(key, { ...followed, followers: followed.followers - 1 });
			return;
		}
		this.#followed.delete(key);
		if (holdsTab(this.#snapshot(), followed.terminalContextId, followed.tabId)) {
			this.#onChange(true);
		}
	}
}

function tabKey(terminalContextId: string, tabId: string): string {
	return `${terminalContextId}:${tabId}`;
}

function holdsTab(snapshot: WorkbenchSnapshot, terminalContextId: string, tabId: string): boolean {
	return snapshot.terminalContexts.some(
		(context) =>
			context.terminalContextId === terminalContextId &&
			context.tabs.some((tab) => tab.tabId === tabId),
	);
}
