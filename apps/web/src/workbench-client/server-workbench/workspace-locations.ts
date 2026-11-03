import type { WorkbenchWorkspaceGit, WorkbenchWorkspaceLocation } from '@orchbee/workbench';
import type { WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';

/**
 * Where each Workspace stands and what Git last said there, kept by the TUI's rules so every Client
 * shows the same row: `checkpoint.rs` for carrying facts across snapshots, and `workspace_sidebar.rs`
 * for the folder a row names.
 */

type SnapshotDirectory = WorkbenchSnapshot['navigation']['directories'][number];
type WorkspaceId = WorkbenchSnapshot['navigation']['workspaces'][number]['workspaceId'];

/** Directories whose last Git answer a Client keeps for an immediate paint on return. */
const MAXIMUM_KNOWN_GIT = 64;

/**
 * How long a row may keep what it settled on while it waits for Git. An answer normally takes about
 * 200 ms and a retried read as long again; the bound stays far below a Git read's budget, so a hung
 * read never freezes a row that already knows its new name and folder.
 */
const WORKSPACE_LOCATION_HOLD_MILLISECONDS = 1_000;

/**
 * What a row keeps presenting, as one settled whole, while Git has not answered for the directory it
 * moved to: a name that moved with the directory waits for the line below it, so the row changes once
 * and never mixes one directory's name with another's facts. A further move keeps the first hold.
 */
interface HeldLocation {
	readonly deadline: number;
	readonly folder: string | null;
	readonly git: WorkbenchWorkspaceGit | null;
	readonly name: string;
}

interface LocationRow {
	/** With `terminalDirectory`, fences reuse of the row's Git facts. */
	readonly directory: SnapshotDirectory | null;
	readonly git: WorkbenchWorkspaceGit | null;
	readonly held: HeldLocation | null;
	readonly name: string;
	/** The first terminal's directory while it differs from `directory`. */
	readonly terminalDirectory: string | null;
}

/** What a Client knows of its Workspaces' locations. */
export interface WorkspaceLocationState {
	/** The last Git answers by directory, oldest first. Hints for presentation only. */
	readonly known: readonly (readonly [path: string, git: WorkbenchWorkspaceGit])[];
	/** The snapshot revision the rows follow, which fences every Git answer. */
	readonly revision: string | null;
	readonly rows: ReadonlyMap<WorkspaceId, LocationRow>;
}

/** One Workspace's Git read: facts observed at a revision, or no answer for now. */
export type WorkspaceGitReading =
	| {
			readonly kind: 'observed';
			readonly git: WorkbenchWorkspaceGit;
			readonly revision: string;
	  }
	| { readonly kind: 'unanswered' };

/** A Workspace Git can be asked about, the directory its answer would describe, and whether one did. */
export interface WorkspaceGitSource {
	readonly answered: boolean;
	readonly source: string;
	readonly workspaceId: WorkspaceId;
}

export const NO_WORKSPACE_LOCATIONS: WorkspaceLocationState = {
	known: [],
	revision: null,
	rows: new Map(),
};

/** The directory a row's Git facts describe: the first terminal's, otherwise the Workspace's own. */
function sourceOf(row: LocationRow): string | null {
	return row.terminalDirectory ?? row.directory?.path ?? null;
}

function sameSource(left: LocationRow, right: LocationRow): boolean {
	const a = left.directory;
	const b = right.directory;
	return (
		left.terminalDirectory === right.terminalDirectory &&
		(a === b ||
			(a !== null &&
				b !== null &&
				a.directoryId === b.directoryId &&
				a.workerId === b.workerId &&
				a.path === b.path))
	);
}

/** The last Git answer for `path`: a current row's, otherwise an earlier one's. */
function gitAt(state: WorkspaceLocationState, path: string): WorkbenchWorkspaceGit | null {
	for (const row of state.rows.values()) {
		if (row.git !== null && sourceOf(row) === path) return row.git;
	}
	return state.known.find(([known]) => known === path)?.[1] ?? null;
}

/**
 * The same source keeps its facts, and a rename under it ends a hold, because whoever renamed it
 * expects to see that at once. A changed source takes the last answer for its directory, or else
 * holds what the row last settled on.
 */
function carry(
	row: LocationRow,
	previous: LocationRow | undefined,
	state: WorkspaceLocationState,
	now: number,
): LocationRow {
	if (previous !== undefined && sameSource(previous, row)) {
		return {
			...row,
			git: previous.git,
			held: previous.name === row.name ? previous.held : null,
		};
	}
	const source = sourceOf(row);
	const known = source === null ? null : gitAt(state, source);
	if (known !== null) return { ...row, git: known };
	if (previous === undefined) return row;
	return {
		...row,
		held: previous.held ?? {
			deadline: now + WORKSPACE_LOCATION_HOLD_MILLISECONDS,
			folder: sourceOf(previous),
			git: previous.git,
			name: previous.name,
		},
	};
}

/** Follows a published snapshot, carrying each Workspace's Git facts by the rules above. */
export function followWorkspaceSnapshot(
	state: WorkspaceLocationState,
	snapshot: WorkbenchSnapshot,
	now: number,
): WorkspaceLocationState {
	const directories = new Map(
		snapshot.navigation.directories.map((directory) => [directory.directoryId, directory]),
	);
	const rows = new Map<WorkspaceId, LocationRow>();
	for (const workspace of snapshot.navigation.workspaces) {
		const row: LocationRow = {
			directory:
				workspace.primaryDirectoryId === null
					? null
					: (directories.get(workspace.primaryDirectoryId) ?? null),
			git: null,
			held: null,
			name: workspace.label,
			terminalDirectory: workspace.terminalDirectory ?? null,
		};
		rows.set(workspace.workspaceId, carry(row, state.rows.get(workspace.workspaceId), state, now));
	}
	return { known: state.known, revision: snapshot.revision, rows };
}

function remember(
	known: WorkspaceLocationState['known'],
	path: string,
	git: WorkbenchWorkspaceGit,
): WorkspaceLocationState['known'] {
	const index = known.findIndex(([candidate]) => candidate === path);
	const next =
		index < 0
			? [...known, [path, git] as const]
			: known.map((entry, position) => (position === index ? ([path, git] as const) : entry));
	return next.slice(Math.max(0, next.length - MAXIMUM_KNOWN_GIT));
}

/**
 * Takes one Workspace's Git read. Facts count only at the revision the rows follow. No answer ends a
 * hold only in a round that asked every Workspace, which says no answer is coming soon; a read of a
 * few says nothing about the rest.
 */
export function acceptWorkspaceGit(
	state: WorkspaceLocationState,
	workspaceId: WorkspaceId,
	reading: WorkspaceGitReading,
	round: 'complete' | 'partial',
): WorkspaceLocationState {
	const row = state.rows.get(workspaceId);
	if (row === undefined) return state;
	if (reading.kind === 'unanswered') {
		if (round === 'partial' || row.held === null) return state;
		return { ...state, rows: new Map(state.rows).set(workspaceId, { ...row, held: null }) };
	}
	if (reading.revision !== state.revision) return state;
	const source = sourceOf(row);
	return {
		known: source === null ? state.known : remember(state.known, source, reading.git),
		revision: state.revision,
		rows: new Map(state.rows).set(workspaceId, { ...row, git: reading.git, held: null }),
	};
}

/** Forgets every Git fact once reads stop reaching the Server; names and folders stay. */
export function withdrawWorkspaceGit(state: WorkspaceLocationState): WorkspaceLocationState {
	return {
		known: [],
		revision: state.revision,
		rows: new Map(
			Array.from(state.rows, ([workspaceId, row]) => [
				workspaceId,
				{ ...row, git: null, held: null },
			]),
		),
	};
}

/** Every Workspace with a directory, in snapshot order. */
export function workspaceGitSources(state: WorkspaceLocationState): readonly WorkspaceGitSource[] {
	const sources: WorkspaceGitSource[] = [];
	for (const [workspaceId, row] of state.rows) {
		const source = sourceOf(row);
		if (source !== null) sources.push({ answered: row.git !== null, source, workspaceId });
	}
	return sources;
}

/** When the next active hold gives way, or null while none is active. */
export function nextHoldDeadline(state: WorkspaceLocationState, now: number): number | null {
	let next: number | null = null;
	for (const { held } of state.rows.values()) {
		if (held !== null && held.deadline > now && (next === null || held.deadline < next)) {
			next = held.deadline;
		}
	}
	return next;
}

function separator(character: string | undefined): boolean {
	return character === '/' || character === '\\';
}

/**
 * `~` replaces a leading home folder under Windows spelling equivalence: either separator, ASCII case
 * and trailing separators. Null for any other path, which stays verbatim. Mirrors `home_relative` in
 * the TUI sidebar.
 */
function homeRelative(path: string, home: string): string | null {
	const base = home.replace(/[\\/]+$/u, '');
	if (base === '' || path.length < base.length) return null;
	for (let index = 0; index < base.length; index += 1) {
		const left = path[index];
		const right = base[index];
		if (
			left?.replace(/[A-Z]/u, (letter) => letter.toLowerCase()) !==
				right?.replace(/[A-Z]/u, (letter) => letter.toLowerCase()) &&
			!(separator(left) && separator(right))
		) {
			return null;
		}
	}
	const rest = path.slice(base.length);
	return rest === '' || separator(rest[0]) ? `~${rest.replace(/[\\/]+$/u, '')}` : null;
}

/**
 * What each row shows: while a hold lasts, the name, folder and facts it settled on; otherwise its
 * own. A folder under `home` shows as `~`.
 */
export function presentWorkspaceLocations(
	state: WorkspaceLocationState,
	now: number,
	home: string | null,
): ReadonlyMap<string, WorkbenchWorkspaceLocation> {
	const locations = new Map<string, WorkbenchWorkspaceLocation>();
	for (const [workspaceId, row] of state.rows) {
		const held = row.held !== null && now < row.held.deadline ? row.held : null;
		const folder = held === null ? sourceOf(row) : held.folder;
		locations.set(workspaceId, {
			folder: folder === null || home === null ? folder : (homeRelative(folder, home) ?? folder),
			git: held === null ? row.git : held.git,
			heldName: held === null || held.name === row.name ? null : held.name,
		});
	}
	return locations;
}

function sameGit(left: WorkbenchWorkspaceGit | null, right: WorkbenchWorkspaceGit | null): boolean {
	return (
		left === right ||
		(left !== null &&
			right !== null &&
			left.branch === right.branch &&
			left.divergence?.ahead === right.divergence?.ahead &&
			left.divergence?.behind === right.divergence?.behind)
	);
}

/** Whether two presentations show the same rows, so an unchanged one keeps its identity. */
export function sameWorkspaceLocations(
	left: ReadonlyMap<string, WorkbenchWorkspaceLocation>,
	right: ReadonlyMap<string, WorkbenchWorkspaceLocation>,
): boolean {
	if (left.size !== right.size) return false;
	for (const [workspaceId, location] of left) {
		const other = right.get(workspaceId);
		if (
			other === undefined ||
			other.folder !== location.folder ||
			other.heldName !== location.heldName ||
			!sameGit(other.git, location.git)
		) {
			return false;
		}
	}
	return true;
}
