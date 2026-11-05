import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import * as React from 'react';

import type {
	WorkbenchWorkspaceLocation,
	WorkbenchWorkspaceLocationSource,
} from '../../../types/workbench-workspace-location';

/** What a Workspace row shows of where it stands. */
export interface WorkbenchSidebarWorkspaceLocation {
	/** Commits to push; the row shows a positive count. */
	readonly ahead: number;
	/** Commits to pull; the row shows a positive count. */
	readonly behind: number;
	/** The directory, with home shortened to `~`; the second line's tooltip shows it whole. */
	readonly directory: string | null;
	/** The shown counts in words, for the row's accessible name. */
	readonly divergenceLabel: string | null;
	readonly heldName: string | null;
	/** The second line: the branch, otherwise the directory's own folder name, or `~` for home. */
	readonly line: { readonly kind: 'branch' | 'folder'; readonly text: string } | null;
}

const NO_LOCATIONS: ReadonlyMap<string, WorkbenchWorkspaceLocation> = new Map();

/** A path's final folder, so `~/code/orchbee` reads `orchbee` and `~` stays `~`. */
function folderName(path: string): string {
	const parts = path.split(/[/\\]/u).filter((part) => part !== '');
	return parts.at(-1) ?? path;
}

function locationLine(
	location: WorkbenchWorkspaceLocation,
): WorkbenchSidebarWorkspaceLocation['line'] {
	const branch = location.git?.branch ?? null;
	if (branch !== null) return { kind: 'branch', text: branch };
	return location.folder === null ? null : { kind: 'folder', text: folderName(location.folder) };
}

/** Observes Workspace locations while the sidebar shows, and words them for its rows. */
export function useWorkbenchSidebarWorkspaceLocations(
	source: WorkbenchWorkspaceLocationSource | undefined,
): ReadonlyMap<string, WorkbenchSidebarWorkspaceLocation> {
	const intl = useOrchbeeIntl();
	// One subscription per source: a source observes only while subscribed, so resubscribing on every
	// render would restart its reads.
	const store = React.useMemo(
		() => ({
			getSnapshot: () => source?.getSnapshot() ?? NO_LOCATIONS,
			subscribe: (listener: () => void) => source?.subscribe(listener) ?? (() => undefined),
		}),
		[source],
	);
	const locations = React.useSyncExternalStore(
		store.subscribe,
		store.getSnapshot,
		store.getSnapshot,
	);
	return React.useMemo(() => {
		const rows = new Map<string, WorkbenchSidebarWorkspaceLocation>();
		for (const [workspaceId, location] of locations) {
			const ahead = location.git?.divergence?.ahead ?? 0;
			const behind = location.git?.divergence?.behind ?? 0;
			const counts = [
				ahead > 0 ? intl.formatMessage(workbenchMessages.workspaceGitAhead, { count: ahead }) : '',
				behind > 0
					? intl.formatMessage(workbenchMessages.workspaceGitBehind, { count: behind })
					: '',
			].filter((label) => label !== '');
			rows.set(workspaceId, {
				ahead,
				behind,
				directory: location.folder,
				divergenceLabel: counts.length === 0 ? null : counts.join('. '),
				heldName: location.heldName,
				line: locationLine(location),
			});
		}
		return rows;
	}, [intl, locations]);
}
