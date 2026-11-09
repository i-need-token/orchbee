import type { WorkbenchTerminalMetadataObservation } from '../../workbench';

type CurrentDirectory = WorkbenchTerminalMetadataObservation['currentDirectory'];

/**
 * A title shown for a terminal: as its live surface reported it, which this Client still strips, or
 * as the Server published it, which the Server already stripped and bounded.
 */
export interface ServerTerminalTitle {
	readonly published: boolean;
	readonly text: string;
}

/** What one `presentation-titles` row says about a Presentation's title. */
type ServerPublishedTitleObservation =
	| { readonly kind: 'unavailable' }
	| { readonly kind: 'observed'; readonly title: string | null };

/** The latest `presentation-titles` row for a Presentation, whichever terminal it names. */
export interface ServerPresentationTitleRow {
	/** The Workbench binding key the Server read the title for, or null for no terminal. */
	readonly binding: string | null;
	readonly title: ServerPublishedTitleObservation;
}

/** One Presentation's title as this Client last saw it. */
export interface ServerTerminalTitleCursor {
	/** The latest row, kept until the snapshot names the terminal it was read for. */
	readonly row: ServerPresentationTitleRow | undefined;
	/** Null is the program's own name: it cleared its title, or this terminal never showed one. */
	readonly shown: ServerTerminalTitle | null;
	/** What rows last observed for `terminal`, or undefined while they observed nothing for it. */
	readonly stream: string | null | undefined;
	/** What the live surface last reported for `terminal`, or undefined before it reported. */
	readonly surface: string | null | undefined;
	/** The snapshot's binding key, or null where the Presentation holds no terminal. */
	readonly terminal: string | null;
}

export type ServerTerminalTitleArrival =
	/** A frame the mounted surface committed; it always names the snapshot's current terminal. */
	| {
			readonly currentDirectory: CurrentDirectory;
			readonly kind: 'surface';
			readonly title: string | null;
	  }
	/** One row of a `presentation-titles` stream. */
	| ({ readonly kind: 'stream' } & ServerPresentationTitleRow)
	/** A publication, which can move the Presentation to another terminal. */
	| { readonly kind: 'terminal' };

export interface ServerTerminalTitleMerge {
	/** What to apply to the Presentation, or null where nothing it shows changes. */
	readonly applied: {
		readonly currentDirectory: CurrentDirectory;
		readonly title: ServerTerminalTitle | null;
	} | null;
	readonly cursor: ServerTerminalTitleCursor | undefined;
}

const IGNORED_DIRECTORY: CurrentDirectory = Object.freeze({ kind: 'ignored' });

/**
 * The naming rule both Clients share for one Presentation. A title arrives when a source reports a
 * value other than the one it last reported for that terminal, and the title that arrived last is
 * shown, whether the surface or the Server delivered it. A row applies only while its binding is the
 * one the snapshot holds, so a row that arrives before its snapshot applies once the snapshot does.
 * `unavailable` keeps what was shown; a new terminal shows the program's name until a title arrives
 * for it. A row carries no directory, so only a surface frame changes it.
 */
export function mergeServerTerminalTitle(
	previous: ServerTerminalTitleCursor | undefined,
	terminal: string | null,
	arrival: ServerTerminalTitleArrival,
): ServerTerminalTitleMerge {
	if (arrival.kind === 'terminal' && previous === undefined) {
		return { applied: null, cursor: undefined };
	}
	let cursor = followTerminal(
		previous ?? {
			row: undefined,
			shown: null,
			stream: undefined,
			surface: undefined,
			terminal,
		},
		terminal,
	);
	if (arrival.kind === 'surface') cursor = observeSurface(cursor, arrival.title);
	if (arrival.kind === 'stream') {
		cursor = { ...cursor, row: { binding: arrival.binding, title: arrival.title } };
	}
	cursor = applyRow(cursor);
	if (arrival.kind === 'surface') {
		return {
			applied: { currentDirectory: arrival.currentDirectory, title: cursor.shown },
			cursor,
		};
	}
	return {
		applied: sameTitle(previous?.shown ?? null, cursor.shown)
			? null
			: { currentDirectory: IGNORED_DIRECTORY, title: cursor.shown },
		cursor,
	};
}

function followTerminal(
	cursor: ServerTerminalTitleCursor,
	terminal: string | null,
): ServerTerminalTitleCursor {
	if (cursor.terminal === terminal) return cursor;
	return { ...cursor, shown: null, stream: undefined, surface: undefined, terminal };
}

function observeSurface(
	cursor: ServerTerminalTitleCursor,
	title: string | null,
): ServerTerminalTitleCursor {
	if (cursor.surface === title) return cursor;
	return {
		...cursor,
		shown: title === null ? null : { published: false, text: title },
		surface: title,
	};
}

function applyRow(cursor: ServerTerminalTitleCursor): ServerTerminalTitleCursor {
	const { row } = cursor;
	if (
		row === undefined ||
		row.binding === null ||
		row.binding !== cursor.terminal ||
		row.title.kind === 'unavailable' ||
		row.title.title === cursor.stream
	) {
		return cursor;
	}
	const { title } = row.title;
	return {
		...cursor,
		shown: title === null ? null : { published: true, text: title },
		stream: title,
	};
}

function sameTitle(before: ServerTerminalTitle | null, after: ServerTerminalTitle | null): boolean {
	return before === null || after === null
		? before === after
		: before.published === after.published && before.text === after.text;
}
