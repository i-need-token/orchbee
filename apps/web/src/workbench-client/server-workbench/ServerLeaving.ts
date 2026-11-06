import type {
	WorkbenchPanelNode,
	WorkbenchSnapshot,
	WorkbenchTerminalContext,
} from '@orchbee/workbench-client-protocol';

/**
 * A part of the Server's Workbench that a close takes out of what this window shows while the close
 * progresses: a Tab, a Panel of a Tab, or a Workspace.
 */
export type ServerLeavingMember =
	| {
			readonly kind: 'panel';
			readonly panelId: string;
			readonly tabId: string;
			readonly terminalContextId: string;
	  }
	| { readonly kind: 'tab'; readonly tabId: string; readonly terminalContextId: string }
	| { readonly kind: 'workspace'; readonly workspaceId: string };

/** Whether the window leaves out a member, as {@link ServerLeaving} decides. */
export interface ServerLeavingView {
	readonly panel: (terminalContextId: string, tabId: string, panelId: string) => boolean;
	readonly tab: (terminalContextId: string, tabId: string) => boolean;
	readonly workspace: (workspaceId: string) => boolean;
}

/** What a close tells {@link ServerLeaving} about the members whose close it follows. */
export interface ServerLeave {
	/**
	 * The members' close progresses, and the window follows it until the returned function is called,
	 * once the close settles or is no longer followed. Members that leave together leave in one change.
	 */
	leave(members: readonly ServerLeavingMember[]): () => void;
}

interface FollowedMember {
	/** How many followed closes hold the member out. */
	readonly followers: number;
	readonly member: ServerLeavingMember;
}

/**
 * The members whose progressing close the window follows. As the TUI leaves out a Tab whose close is
 * progressing, the window projects the Server's snapshot without the members it follows, so a Tab, a
 * Panel, or a Workspace leaves the window when its close is accepted instead of when the publication
 * removes it. A Panel leaves as the Server removes it, its sibling taking the split's place; the
 * selection moves on as the publication reconciles. A member shows again once no close of it is
 * followed, which a refused, stopped, unknown or lost close ends with; a completed close has removed
 * it by then.
 */
export class ServerLeaving implements ServerLeave, ServerLeavingView {
	readonly #followed = new Map<string, FollowedMember>();
	readonly #onChange: (returned: boolean) => void;
	readonly #snapshot: () => WorkbenchSnapshot;

	/**
	 * `onChange` hears the members the snapshot holds leave or return, once per change, with whether
	 * one returned, so the snapshot can be projected again.
	 */
	public constructor(snapshot: () => WorkbenchSnapshot, onChange: (returned: boolean) => void) {
		this.#onChange = onChange;
		this.#snapshot = snapshot;
	}

	public readonly panel = (terminalContextId: string, tabId: string, panelId: string): boolean =>
		this.#followed.has(memberKey({ kind: 'panel', panelId, tabId, terminalContextId }));

	public readonly tab = (terminalContextId: string, tabId: string): boolean =>
		this.#followed.has(memberKey({ kind: 'tab', tabId, terminalContextId }));

	public readonly workspace = (workspaceId: string): boolean =>
		this.#followed.has(memberKey({ kind: 'workspace', workspaceId }));

	public leave(members: readonly ServerLeavingMember[]): () => void {
		const snapshot = this.#snapshot();
		let left = false;
		const keys = members.map((member) => {
			const key = memberKey(member);
			const followers = this.#followed.get(key)?.followers ?? 0;
			this.#followed.set(key, { followers: followers + 1, member });
			left ||= followers === 0 && holdsMember(snapshot, member);
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

	/** Forgets each member `snapshot` no longer holds, since identities never return. */
	public prune(snapshot: WorkbenchSnapshot): void {
		for (const [key, { member }] of this.#followed) {
			if (!holdsMember(snapshot, member)) this.#followed.delete(key);
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
			returned ||= holdsMember(snapshot, followed.member);
		}
		if (returned) this.#onChange(true);
	}
}

function memberKey(member: ServerLeavingMember): string {
	switch (member.kind) {
		case 'panel':
			return `panel:${member.terminalContextId}:${member.tabId}:${member.panelId}`;
		case 'tab':
			return `tab:${member.terminalContextId}:${member.tabId}`;
		case 'workspace':
			return `workspace:${member.workspaceId}`;
	}
}

function holdsMember(snapshot: WorkbenchSnapshot, member: ServerLeavingMember): boolean {
	switch (member.kind) {
		case 'workspace':
			return snapshot.navigation.workspaces.some(
				(workspace) => workspace.workspaceId === member.workspaceId,
			);
		case 'tab':
			return findTab(snapshot, member.terminalContextId, member.tabId) !== undefined;
		case 'panel': {
			const tab = findTab(snapshot, member.terminalContextId, member.tabId);
			return tab !== undefined && containsPanel(tab.root, member.panelId);
		}
	}
}

function findTab(snapshot: WorkbenchSnapshot, terminalContextId: string, tabId: string) {
	return snapshot.terminalContexts
		.find((context: WorkbenchTerminalContext) => context.terminalContextId === terminalContextId)
		?.tabs.find((tab) => tab.tabId === tabId);
}

function containsPanel(node: WorkbenchPanelNode, panelId: string): boolean {
	return node.kind === 'panel'
		? node.panelId === panelId
		: node.children.some((slot) => containsPanel(slot.node, panelId));
}
