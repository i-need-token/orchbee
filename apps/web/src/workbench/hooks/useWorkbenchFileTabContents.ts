import * as React from 'react';

import {
	FileTabContentRegistry,
	type FileOverlayStore,
	type FileTabContents,
} from '../../file-overlay';
import type { FileSessionRegistry } from '../../file-session';
import type { WorkbenchWorkspace } from '../../project-workspace';

interface UseWorkbenchFileTabContentsOptions {
	/** The Client's file sessions; without them no tab shows content. */
	readonly sessions: FileSessionRegistry | null;
	/** The shown Workspace's overlay while it shows; null otherwise. */
	readonly store: FileOverlayStore | null;
	/** The active Workspace; null outside a Workspace. */
	readonly workspaceId: string | null;
	/** The window's Workspaces, whose tabs' content lives while they exist. */
	readonly workspaces: readonly WorkbenchWorkspace[];
}

/**
 * Keeps the content behind each Workspace's file tabs for this Workbench while its Workspace exists,
 * and gives the shown overlay its Workspace's. New file sessions, as a new connection brings, start
 * every Workspace's content again.
 */
export function useWorkbenchFileTabContents({
	sessions,
	store,
	workspaceId,
	workspaces,
}: UseWorkbenchFileTabContentsOptions): FileTabContents | null {
	const registry = React.useMemo(
		() => (sessions === null ? null : new FileTabContentRegistry(sessions)),
		[sessions],
	);
	React.useEffect(() => () => registry?.dispose(), [registry]);
	React.useEffect(() => {
		registry?.retain(new Set(workspaces.map((workspace) => workspace.id)));
	}, [registry, workspaces]);
	return registry === null || store === null || workspaceId === null
		? null
		: registry.of(workspaceId, store);
}
