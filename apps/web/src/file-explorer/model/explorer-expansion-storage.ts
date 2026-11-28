import { z } from 'zod';

import { EXPLORER_EXPANSION_LIMIT } from '../constants/explorer-expansion-limit';

/**
 * The folders each Workspace's Explorer had expanded, kept on this device so they open again in the
 * next run; losing the record only leaves them collapsed. It holds paths of folders the Explorer
 * shows anyway, never file content, within the bounds below.
 */
const EXPLORER_EXPANSION_STORAGE_KEY = 'orchbee:file-explorer:expanded-folders:v1';
/** The Workspaces the record keeps, most recently changed first. */
const MAXIMUM_STORED_WORKSPACES = 50;
/** Longer than any path a filesystem names, so such a folder is not saved. */
const MAXIMUM_PATH_LENGTH = 4096;
/**
 * The serialized record's length at most, a small share of the storage every record of the origin
 * shares: past it the least recently changed Workspaces go, then the saved one's oldest folders.
 */
const MAXIMUM_RECORD_LENGTH = 500_000;

const StoredExpansionSchema = z.strictObject({
	workspaces: z
		.array(
			z.strictObject({
				/** Folder comparison keys, least recently expanded first. */
				expanded: z.array(z.string().min(1).max(MAXIMUM_PATH_LENGTH)).max(EXPLORER_EXPANSION_LIMIT),
				workspaceId: z.string().regex(/^[0-9a-f]{32}$/u),
			}),
		)
		.max(MAXIMUM_STORED_WORKSPACES),
});

type StoredExpansion = z.output<typeof StoredExpansionSchema>;
type StoredWorkspace = StoredExpansion['workspaces'][number];

export type ExplorerExpansionStorage = Pick<Storage, 'getItem' | 'setItem'>;

const RECORD_FRAME_LENGTH = JSON.stringify({ workspaces: [] }).length;

function readStored(storage: ExplorerExpansionStorage): StoredExpansion | null {
	try {
		const serializedValue = storage.getItem(EXPLORER_EXPANSION_STORAGE_KEY);
		if (serializedValue === null) return null;
		const result = StoredExpansionSchema.safeParse(JSON.parse(serializedValue));
		return result.success ? result.data : null;
	} catch {
		return null;
	}
}

/** The Workspace being saved, keeping its most recently expanded folders while it alone is too long. */
function fitted(workspace: StoredWorkspace): StoredWorkspace {
	let { expanded } = workspace;
	const lengthOf = (folders: readonly string[]): number =>
		RECORD_FRAME_LENGTH +
		JSON.stringify({ expanded: folders, workspaceId: workspace.workspaceId }).length;
	while (expanded.length > 0 && lengthOf(expanded) > MAXIMUM_RECORD_LENGTH) {
		expanded = expanded.slice(Math.ceil(expanded.length / 2));
	}
	return { expanded, workspaceId: workspace.workspaceId };
}

/** The Workspaces that fit the record's length, most recently changed first. */
function withinLength(
	saved: StoredWorkspace,
	others: readonly StoredWorkspace[],
): StoredWorkspace[] {
	const first = fitted(saved);
	const kept = [first];
	let length = RECORD_FRAME_LENGTH + JSON.stringify(first).length;
	for (const workspace of others.slice(0, MAXIMUM_STORED_WORKSPACES - 1)) {
		length += JSON.stringify(workspace).length + 1;
		if (length > MAXIMUM_RECORD_LENGTH) break;
		kept.push(workspace);
	}
	return kept;
}

/** A Workspace's expanded folders as this device last saved them, least recently expanded first. */
export function readExplorerExpansion(
	storage: ExplorerExpansionStorage | undefined,
	workspaceId: string,
): readonly string[] {
	if (storage === undefined) return [];
	const stored = readStored(storage)?.workspaces.find((entry) => entry.workspaceId === workspaceId);
	return stored?.expanded ?? [];
}

/** Saves a Workspace's expanded folders as the most recently changed, within the record's bounds. */
export function writeExplorerExpansion(
	storage: ExplorerExpansionStorage | undefined,
	workspaceId: string,
	expanded: readonly string[],
): void {
	if (storage === undefined) return;
	const others = (readStored(storage)?.workspaces ?? []).filter(
		(entry) => entry.workspaceId !== workspaceId,
	);
	const saved = {
		expanded: expanded
			.filter((folder) => folder.length > 0 && folder.length <= MAXIMUM_PATH_LENGTH)
			.slice(-EXPLORER_EXPANSION_LIMIT),
		workspaceId,
	};
	const result = StoredExpansionSchema.safeParse({ workspaces: withinLength(saved, others) });
	if (!result.success) return;
	try {
		storage.setItem(EXPLORER_EXPANSION_STORAGE_KEY, JSON.stringify(result.data));
	} catch {
		// Device-local view memory is best effort and must not block the interface.
	}
}
