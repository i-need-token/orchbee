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
export type ServerPublishedTitleObservation =
	| { readonly kind: 'unavailable' }
	| { readonly kind: 'observed'; readonly title: string | null };

/**
 * One Presentation's title as this Client last saw it. A Presentation's terminal is named twice: by
 * the Workbench snapshot's binding, which keys the live surface, and by the Host incarnation the
 * Server names in a title row. The Server never moves a binding to another incarnation, so each
 * names the other one to one; only their arrival order differs, and `pairing` records which side has
 * already moved to the next terminal.
 */
export interface ServerTerminalTitleCursor {
	/** The incarnation the latest title row named, or null before any row named one. */
	readonly incarnation: string | null;
	/**
	 * `paired` when `incarnation` is `terminal`'s; `awaiting-row` when the snapshot moved to another
	 * terminal first; `awaiting-terminal` when a row named another incarnation first.
	 */
	readonly pairing: 'awaiting-row' | 'awaiting-terminal' | 'paired';
	/** Null is the program's own name: it cleared its title, or this terminal never showed one. */
	readonly shown: ServerTerminalTitle | null;
	/** What `incarnation`'s rows last observed, or undefined while they observed nothing. */
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
	/** One row of a `presentation-titles` stream; a null incarnation names no terminal. */
	| {
			readonly incarnation: string | null;
			readonly kind: 'stream';
			readonly title: ServerPublishedTitleObservation;
	  }
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
 * shown, whether the surface or the Server delivered it. A row naming an incarnation that is not the
 * Presentation's current terminal changes nothing. `unavailable` keeps what was shown; a new
 * terminal shows the program's name until a title arrives for it, except where no row has ever
 * named one, which keeps what the surface showed until it reports again. A row carries no directory,
 * so only a surface frame changes it.
 */
export function mergeServerTerminalTitle(
	previous: ServerTerminalTitleCursor | undefined,
	terminal: string | null,
	arrival: ServerTerminalTitleArrival,
): ServerTerminalTitleMerge {
	if (arrival.kind === 'terminal' && previous === undefined) {
		return { applied: null, cursor: undefined };
	}
	const current = followTerminal(
		previous ?? {
			incarnation: null,
			pairing: 'awaiting-row',
			shown: null,
			stream: undefined,
			surface: undefined,
			terminal,
		},
		terminal,
	);
	switch (arrival.kind) {
		case 'surface': {
			const cursor = observeSurface(current, arrival.title);
			return {
				applied: { currentDirectory: arrival.currentDirectory, title: cursor.shown },
				cursor,
			};
		}
		case 'stream':
			return arrival.incarnation === null
				? { applied: null, cursor: previous }
				: changed(previous, observeStream(current, arrival.incarnation, arrival.title));
		case 'terminal':
			return changed(previous, current);
	}
}

function changed(
	previous: ServerTerminalTitleCursor | undefined,
	cursor: ServerTerminalTitleCursor,
): ServerTerminalTitleMerge {
	const before = previous?.shown ?? null;
	const after = cursor.shown;
	const same =
		before === null || after === null
			? before === after
			: before.published === after.published && before.text === after.text;
	return {
		applied: same ? null : { currentDirectory: IGNORED_DIRECTORY, title: after },
		cursor,
	};
}

function followTerminal(
	cursor: ServerTerminalTitleCursor,
	terminal: string | null,
): ServerTerminalTitleCursor {
	if (cursor.terminal === terminal) return cursor;
	// Without a row, nothing says which terminal a title belongs to, so the surface's title stays
	// until the new terminal's surface reports, as it always has.
	if (cursor.incarnation === null) return { ...cursor, terminal };
	if (cursor.pairing === 'awaiting-terminal') {
		return {
			...cursor,
			pairing: 'paired',
			shown: publishedTitle(cursor.stream),
			surface: undefined,
			terminal,
		};
	}
	return { ...cursor, pairing: 'awaiting-row', shown: null, surface: undefined, terminal };
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

function observeStream(
	cursor: ServerTerminalTitleCursor,
	incarnation: string,
	title: ServerPublishedTitleObservation,
): ServerTerminalTitleCursor {
	const observed = title.kind === 'observed' ? title.title : undefined;
	if (incarnation !== cursor.incarnation) {
		if (cursor.incarnation === null) {
			// The first row names the terminal the snapshot holds now, which may have shown a title.
			return {
				...cursor,
				incarnation,
				pairing: 'paired',
				shown: observed === undefined ? cursor.shown : publishedTitle(observed),
				stream: observed,
			};
		}
		if (cursor.pairing === 'awaiting-row') {
			return {
				...cursor,
				incarnation,
				pairing: 'paired',
				shown: publishedTitle(observed),
				stream: observed,
			};
		}
		// The Server moved to the next terminal before this Client's snapshot did.
		return { ...cursor, incarnation, pairing: 'awaiting-terminal', stream: observed };
	}
	// The previous terminal's row, which a later row replaces.
	if (cursor.pairing === 'awaiting-row') return cursor;
	if (observed === undefined || observed === cursor.stream) return cursor;
	return cursor.pairing === 'paired'
		? { ...cursor, shown: publishedTitle(observed), stream: observed }
		: { ...cursor, stream: observed };
}

function publishedTitle(title: string | null | undefined): ServerTerminalTitle | null {
	return title === null || title === undefined ? null : { published: true, text: title };
}
