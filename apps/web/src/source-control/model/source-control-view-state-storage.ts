import { z } from 'zod';

import {
	DEFAULT_SCM_SECTIONS_STATE,
	DEFAULT_SCM_VIEW_STATE,
	SCM_VIEW_MODES,
	SCM_VIEW_SORT_KEYS,
	type ScmSectionsState,
	type ScmViewState,
} from './scm-view-state';

const SOURCE_CONTROL_VIEW_STATE_STORAGE_KEY = 'orchbee:source-control:view-state:v1';
/** The Workspaces the record keeps, most recently changed first. */
const MAXIMUM_STORED_WORKSPACES = 50;
/** No column is this tall; a larger stored height is not one the person dragged. */
const MAXIMUM_SECTION_SIZE = 10_000;

const StoredViewStateSchema = z.strictObject({
	sections: z.strictObject({
		changesCollapsed: z.boolean(),
		repositoriesCollapsed: z.boolean(),
		repositoriesSize: z.number().int().min(0).max(MAXIMUM_SECTION_SIZE).nullable(),
	}),
	workspaces: z
		.array(
			z.strictObject({
				mode: z.enum(SCM_VIEW_MODES),
				sortKey: z.enum(SCM_VIEW_SORT_KEYS),
				workspaceId: z.string().regex(/^[0-9a-f]{32}$/u),
			}),
		)
		.max(MAXIMUM_STORED_WORKSPACES),
});

type StoredViewState = z.output<typeof StoredViewStateSchema>;

export type SourceControlViewStateStorage = Pick<Storage, 'getItem' | 'setItem'>;

function readStored(storage: SourceControlViewStateStorage | undefined): StoredViewState | null {
	if (storage === undefined) return null;
	try {
		const serializedValue = storage.getItem(SOURCE_CONTROL_VIEW_STATE_STORAGE_KEY);
		if (serializedValue === null) return null;
		const result = StoredViewStateSchema.safeParse(JSON.parse(serializedValue));
		return result.success ? result.data : null;
	} catch {
		return null;
	}
}

function writeStored(storage: SourceControlViewStateStorage, value: StoredViewState): void {
	const result = StoredViewStateSchema.safeParse(value);
	if (!result.success) return;
	try {
		storage.setItem(SOURCE_CONTROL_VIEW_STATE_STORAGE_KEY, JSON.stringify(result.data));
	} catch {
		// Device-local view memory is best effort and must not block the interface.
	}
}

/** The view mode and sort a Workspace's Changes last committed, or VS Code's defaults. */
export function readSourceControlViewState(
	storage: SourceControlViewStateStorage | undefined,
	workspaceId: string,
): ScmViewState {
	const stored = readStored(storage)?.workspaces.find((entry) => entry.workspaceId === workspaceId);
	return stored === undefined
		? DEFAULT_SCM_VIEW_STATE
		: { mode: stored.mode, sortKey: stored.sortKey };
}

/** Keeps a Workspace's committed choice first, dropping the least recently changed past the bound. */
export function writeSourceControlViewState(
	storage: SourceControlViewStateStorage | undefined,
	workspaceId: string,
	state: ScmViewState,
): void {
	if (storage === undefined) return;
	const stored = readStored(storage);
	const others = (stored?.workspaces ?? []).filter((entry) => entry.workspaceId !== workspaceId);
	writeStored(storage, {
		sections: stored?.sections ?? DEFAULT_SCM_SECTIONS_STATE,
		workspaces: [{ ...state, workspaceId }, ...others].slice(0, MAXIMUM_STORED_WORKSPACES),
	});
}

/** The sections' collapsed states and the Repositories height, which every Workspace shares. */
export function readSourceControlSections(
	storage: SourceControlViewStateStorage | undefined,
): ScmSectionsState {
	return readStored(storage)?.sections ?? DEFAULT_SCM_SECTIONS_STATE;
}

export function writeSourceControlSections(
	storage: SourceControlViewStateStorage | undefined,
	sections: ScmSectionsState,
): void {
	if (storage === undefined) return;
	writeStored(storage, { sections, workspaces: readStored(storage)?.workspaces ?? [] });
}
