import { z } from 'zod';

import {
	restoreFileOverlay,
	snapshotFileOverlay,
	type FileOverlaySnapshot,
	type FileOverlayState,
} from './file-overlay-state';

/**
 * Each Workspace's overlay, kept on this device so that its tabs come back in the next run with their
 * states and its tier. It holds file paths and tab states, never file content, within the bounds
 * below; losing it only starts the overlays closed.
 */
const FILE_OVERLAY_TABS_STORAGE_KEY = 'orchbee:file-overlay:tabs:v1';
/** The Workspaces the record keeps, most recently changed first. */
const MAXIMUM_STORED_WORKSPACES = 50;
/** The tabs it keeps of one Workspace: the most recently active, in row order. */
const MAXIMUM_STORED_TABS = 200;
/** The longest path the backend sends. */
const MAXIMUM_PATH_LENGTH = 4096;
/** Lowercasing, which makes a comparison key where case is ignored, can lengthen a path. */
const MAXIMUM_KEY_LENGTH = 2 * MAXIMUM_PATH_LENGTH;
/**
 * The serialized record's length at most, a small share of the storage every record of the origin
 * shares: past it the least recently changed Workspaces go.
 */
const MAXIMUM_RECORD_LENGTH = 500_000;

const StoredWorkspaceSchema = z.strictObject({
	activeIndex: z
		.number()
		.int()
		.min(0)
		.max(MAXIMUM_STORED_TABS - 1),
	mode: z.enum(['open', 'pip']),
	previewIndex: z
		.number()
		.int()
		.min(0)
		.max(MAXIMUM_STORED_TABS - 1)
		.nullable(),
	restoreTier: z.enum(['default', 'left-expand']),
	stickyCount: z.number().int().min(0).max(MAXIMUM_STORED_TABS),
	/** The row, left to right: each file by its path and its comparison key. */
	tabs: z
		.array(
			z.strictObject({
				key: z.string().min(1).max(MAXIMUM_KEY_LENGTH),
				path: z.string().min(1).max(MAXIMUM_PATH_LENGTH),
			}),
		)
		.min(1)
		.max(MAXIMUM_STORED_TABS),
	tier: z.enum(['default', 'full', 'left-expand']),
	workspaceId: z.string().regex(/^[0-9a-f]{32}$/u),
});

const StoredTabsSchema = z.strictObject({
	workspaces: z.array(StoredWorkspaceSchema).max(MAXIMUM_STORED_WORKSPACES),
});

type StoredTabs = z.output<typeof StoredTabsSchema>;
type StoredWorkspace = z.output<typeof StoredWorkspaceSchema>;

export type FileOverlayTabsStorage = Pick<Storage, 'getItem' | 'setItem'>;

const RECORD_FRAME_LENGTH = JSON.stringify({ workspaces: [] }).length;

function readStored(storage: FileOverlayTabsStorage | undefined): StoredTabs | null {
	if (storage === undefined) return null;
	try {
		const serializedValue = storage.getItem(FILE_OVERLAY_TABS_STORAGE_KEY);
		if (serializedValue === null) return null;
		const result = StoredTabsSchema.safeParse(JSON.parse(serializedValue));
		return result.success ? result.data : null;
	} catch {
		return null;
	}
}

function writeStored(
	storage: FileOverlayTabsStorage,
	workspaces: readonly StoredWorkspace[],
): void {
	const result = StoredTabsSchema.safeParse({ workspaces });
	if (!result.success) return;
	try {
		storage.setItem(FILE_OVERLAY_TABS_STORAGE_KEY, JSON.stringify(result.data));
	} catch {
		// Device-local layout memory is best effort and must not block the interface.
	}
}

function snapshotOf(stored: StoredWorkspace): FileOverlaySnapshot {
	return {
		kind: stored.mode,
		restoreTier: stored.restoreTier,
		tabs: {
			activeIndex: stored.activeIndex,
			inputs: stored.tabs.map((tab) => ({
				kind: 'file',
				resource: { key: tab.key, path: tab.path },
			})),
			previewIndex: stored.previewIndex,
			stickyCount: stored.stickyCount,
		},
		tier: stored.tier,
	};
}

/** A Workspace's entry for its overlay, or null when the overlay is closed or the record cannot hold it. */
function entryOf(workspaceId: string, state: FileOverlayState): StoredWorkspace | null {
	const snapshot = snapshotFileOverlay(state, MAXIMUM_STORED_TABS);
	if (snapshot === null) return null;
	const result = StoredWorkspaceSchema.safeParse({
		activeIndex: snapshot.tabs.activeIndex,
		mode: snapshot.kind,
		previewIndex: snapshot.tabs.previewIndex,
		restoreTier: snapshot.restoreTier,
		stickyCount: snapshot.tabs.stickyCount,
		tabs: snapshot.tabs.inputs.map(({ resource }) => ({ key: resource.key, path: resource.path })),
		tier: snapshot.tier,
		workspaceId,
	});
	return result.success ? result.data : null;
}

/** The saved Workspace first, unless it alone is too long, then the others while the record fits. */
function withinLength(
	saved: StoredWorkspace,
	others: readonly StoredWorkspace[],
): StoredWorkspace[] {
	const fits = RECORD_FRAME_LENGTH + JSON.stringify(saved).length <= MAXIMUM_RECORD_LENGTH;
	const kept: StoredWorkspace[] = [];
	let length = RECORD_FRAME_LENGTH;
	for (const workspace of fits ? [saved, ...others] : others) {
		if (kept.length === MAXIMUM_STORED_WORKSPACES) break;
		length += JSON.stringify(workspace).length + (kept.length === 0 ? 0 : 1);
		if (length > MAXIMUM_RECORD_LENGTH) break;
		kept.push(workspace);
	}
	return kept;
}

/**
 * The overlay a Workspace had when this device last kept it, as a window that offers left-expand or
 * not restores it; null, when there is none it can trust, starts it closed.
 */
export function readFileOverlayTabs(
	storage: FileOverlayTabsStorage | undefined,
	workspaceId: string,
	leftExpandOffered: boolean,
): FileOverlayState | null {
	const stored = readStored(storage)?.workspaces.find((entry) => entry.workspaceId === workspaceId);
	return stored === undefined ? null : restoreFileOverlay(snapshotOf(stored), leftExpandOffered);
}

/**
 * Keeps a Workspace's overlay after a change from `previous` as the most recently changed; a change to
 * nothing the record holds, as a tab gaining unsaved edits, writes nothing, and a closed overlay
 * leaves the record.
 */
export function writeFileOverlayTabs(
	storage: FileOverlayTabsStorage | undefined,
	workspaceId: string,
	state: FileOverlayState,
	previous: FileOverlayState,
): void {
	if (storage === undefined) return;
	const entry = entryOf(workspaceId, state);
	if (JSON.stringify(entry) === JSON.stringify(entryOf(workspaceId, previous))) return;
	const others = (readStored(storage)?.workspaces ?? []).filter(
		(other) => other.workspaceId !== workspaceId,
	);
	writeStored(storage, entry === null ? others : withinLength(entry, others));
}

/** Drops what the record keeps of Workspaces that no longer exist. */
export function forgetFileOverlayTabs(
	storage: FileOverlayTabsStorage | undefined,
	workspaceIds: readonly string[],
): void {
	if (storage === undefined || workspaceIds.length === 0) return;
	const stored = readStored(storage);
	if (stored === null) return;
	const kept = stored.workspaces.filter((entry) => !workspaceIds.includes(entry.workspaceId));
	if (kept.length < stored.workspaces.length) writeStored(storage, kept);
}
