import type { FileSessionRegistry } from '../../file-session';
import type { FileOverlayStore } from './FileOverlayRegistry';
import { FileTabContents } from './FileTabContents';

/**
 * The content behind the tabs of one Workbench's Workspaces, each holding the Workspace's file
 * session from the registry while it shows a file. Each lives while its Workspace's overlay does.
 */
export class FileTabContentRegistry {
	private readonly contents = new Map<string, FileTabContents>();
	private readonly sessions: FileSessionRegistry;

	public constructor(sessions: FileSessionRegistry) {
		this.sessions = sessions;
	}

	/**
	 * The content behind a Workspace's tabs in its overlay `store`, created the first time anything
	 * asks for it; a new overlay for the Workspace starts new content.
	 */
	public of(workspaceId: string, store: FileOverlayStore): FileTabContents {
		const current = this.contents.get(workspaceId);
		if (current?.store === store) return current;
		current?.dispose();
		const contents = new FileTabContents(store, () => this.sessions.hold(workspaceId));
		this.contents.set(workspaceId, contents);
		return contents;
	}

	/**
	 * Drops the content of Workspaces that are gone, except where tabs hold unsaved edits, which the
	 * overlay keeps until the person saves or discards them.
	 */
	public retain(workspaceIds: ReadonlySet<string>): void {
		for (const [workspaceId, contents] of this.contents) {
			if (workspaceIds.has(workspaceId)) continue;
			if (contents.store.state.getState().tabs.tabs.some((tab) => tab.unsaved)) continue;
			this.contents.delete(workspaceId);
			contents.dispose();
		}
	}

	/** Lets go of every Workspace's content, as the Workbench or its file sessions go. */
	public dispose(): void {
		for (const contents of this.contents.values()) contents.dispose();
		this.contents.clear();
	}
}
