import type { WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

/** Whether the window leaves a Tab out of what it shows, as {@link ServerLeavingTabs} decides. */
export type ServerTabLeaving = (terminalContextId: string, tabId: string) => boolean;

/** What a close tells {@link ServerLeavingTabs} about the Tabs whose close it follows. */
export interface ServerTabLeave {
	/**
	 * The Tabs' close progresses, and the window follows it until it calls the returned function,
	 * once the close settles or is no longer followed. Tabs that leave together leave in one change.
	 */
	leave(terminalContextId: string, tabIds: readonly string[]): () => void;
}

interface FollowedTab {
	/** How many followed closes hold the Tab out. */
	readonly followers: number;
	readonly tabId: string;
	readonly terminalContextId: string;
}

/**
 * The Tabs whose progressing close the window follows. As the TUI leaves out a Tab whose close is
 * progressing, the window projects the Server's snapshot without them, so a Tab leaves the strip
 * when its close progresses instead of when the publication removes it, and the selection moves
 * on as the publication reconciles. Every member of a set the Server accepts leaves in one change,
 * so a selected member gives way at once to the next Tab outside the set, else the previous one. A
 * Tab shows again once no close of it is followed, which a refused, stopped, unknown or lost close
 * ends with; a completed close has removed it by then.
 */
export class ServerLeavingTabs implements ServerTabLeave {
	readonly #followed = new Map<string, FollowedTab>();
	readonly #onChange: (returned: boolean) => void;
	readonly #snapshot: () => WorkbenchSnapshot;

	/**
	 * `onChange` hears the Tabs the snapshot holds leave or return, once per change, with whether one
	 * returned, so the snapshot can be projected again.
	 */
	public constructor(snapshot: () => WorkbenchSnapshot, onChange: (returned: boolean) => void) {
		this.#onChange = onChange;
		this.#snapshot = snapshot;
	}

	public readonly leaving: ServerTabLeaving = (terminalContextId, tabId) =>
		this.#followed.has(tabKey(terminalContextId, tabId));

	public leave(terminalContextId: string, tabIds: readonly string[]): () => void {
		const snapshot = this.#snapshot();
		let left = false;
		const keys = tabIds.map((tabId) => {
			const key = tabKey(terminalContextId, tabId);
			const followers = this.#followed.get(key)?.followers ?? 0;
			this.#followed.set(key, { followers: followers + 1, tabId, terminalContextId });
			left ||= followers === 0 && holdsTab(snapshot, terminalContextId, tabId);
			return key;
		});
		if (left) this.#onChange(false);
		let followed = true;
		return () => {
			if (!followed) return;
			followed = false;
			this.#stay(keys);
		};
	}

	/** Forgets each Tab `snapshot` no longer holds, since Tab IDs never return. */
	public prune(snapshot: WorkbenchSnapshot): void {
		for (const [key, { tabId, terminalContextId }] of this.#followed) {
			if (!holdsTab(snapshot, terminalContextId, tabId)) this.#followed.delete(key);
		}
	}

	#stay(keys: readonly string[]): void {
		const snapshot = this.#snapshot();
		let returned = false;
		for (const key of keys) {
			const followed = this.#followed.get(key);
			if (followed === undefined) continue;
			if (followed.followers > 1) {
				this.#followed.set(key, { ...followed, followers: followed.followers - 1 });
				continue;
			}
			this.#followed.delete(key);
			returned ||= holdsTab(snapshot, followed.terminalContextId, followed.tabId);
		}
		if (returned) this.#onChange(true);
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
