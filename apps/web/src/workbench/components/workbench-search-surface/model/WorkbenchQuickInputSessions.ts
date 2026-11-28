/** The icon a pick's item shows before its label: VS Code's icon for a file. */
interface WorkbenchQuickPickIcon {
	readonly kind: 'file';
	readonly name: string;
	readonly path: string;
}

/** A row of a pick session: an item to choose, or a separator with an optional label. */
type WorkbenchQuickPickEntry<Value> =
	| {
			readonly description?: string | undefined;
			readonly icon?: WorkbenchQuickPickIcon | undefined;
			readonly id: string;
			/** Shows the item in italics, as VS Code shows an editor in preview. */
			readonly italic?: boolean | undefined;
			readonly kind: 'item';
			readonly label: string;
			readonly value: Value;
	  }
	| { readonly kind: 'separator'; readonly label?: string | undefined };

/** Which item a pick makes active first. */
type WorkbenchQuickPickActivation = 'first' | 'last' | 'second';

/** The modifier keys one chord holds, among the keys that run a quick navigate. */
export interface WorkbenchQuickNavigateKeys {
	readonly altKey: boolean;
	readonly ctrlKey: boolean;
	readonly metaKey: boolean;
	readonly shiftKey: boolean;
}

/** A key let go while a pick has the keys, and the modifier keys still held. */
interface WorkbenchQuickInputKeyUp {
	readonly altKey: boolean;
	readonly ctrlKey: boolean;
	/** The key let go, as `KeyboardEvent.key` names it. */
	readonly key: string;
	readonly metaKey: boolean;
}

/** What every session shows around its rows or its field. */
interface WorkbenchQuickInputFrame {
	/** Shown while the requester is still working, such as loading a later step's items. */
	readonly busy?: boolean | undefined;
	readonly placeholder?: string | undefined;
	readonly step?: { readonly current: number; readonly total: number } | undefined;
	readonly title?: string | undefined;
}

export interface WorkbenchQuickPickRequest<Value> extends WorkbenchQuickInputFrame {
	/**
	 * The item active first: the first, unless this names the second or the last. Without the item it
	 * names, as when a pick that starts on the second item has one, none is active.
	 */
	readonly activation?: WorkbenchQuickPickActivation | undefined;
	readonly entries: readonly WorkbenchQuickPickEntry<Value>[];
	/** Shows no field, as VS Code's quick navigate does when held keys open a pick. */
	readonly hideInput?: boolean | undefined;
	/** Filters on the items' descriptions as well as their labels. */
	readonly matchOnDescription?: boolean | undefined;
	/**
	 * VS Code's quick navigate: the chords that opened the pick. Letting go of a modifier key one of
	 * them holds accepts the active item, once.
	 */
	readonly quickNavigate?: readonly WorkbenchQuickNavigateKeys[] | undefined;
	/** Ends the session as cancelled once it aborts, as when the requester no longer needs a pick. */
	readonly signal?: AbortSignal | undefined;
}

interface WorkbenchQuickInputValidation {
	readonly message: string;
	readonly severity: 'error' | 'info' | 'warning';
}

export interface WorkbenchQuickInputRequest extends WorkbenchQuickInputFrame {
	/** Keeps the session open when focus leaves it, as a credential prompt does; Escape still ends it. */
	readonly ignoreFocusOut?: boolean | undefined;
	readonly password?: boolean | undefined;
	readonly prompt?: string | undefined;
	/**
	 * Checks one value. The signal aborts once the value changes or the session ends, and only the
	 * current value's answer is shown; an answer of `error` severity, or none yet, keeps Enter from
	 * accepting. A rejected check counts as no message.
	 */
	readonly validate?:
		| ((value: string, signal: AbortSignal) => Promise<WorkbenchQuickInputValidation | null>)
		| undefined;
	readonly value?: string | undefined;
}

export type WorkbenchQuickInputOutcome<Value> =
	| { readonly kind: 'accepted'; readonly value: Value }
	| { readonly kind: 'cancelled' };

type WorkbenchQuickPickRow =
	| {
			readonly description?: string | undefined;
			readonly icon?: WorkbenchQuickPickIcon | undefined;
			readonly id: string;
			readonly italic: boolean;
			readonly kind: 'item';
			readonly label: string;
	  }
	| { readonly kind: 'separator'; readonly label?: string | undefined };

/** The current session as the surface renders it, without the requester's values. */
export type WorkbenchQuickInputSession =
	| {
			/** The shown item that Enter accepts, or null while none is active. */
			readonly activeId: string | null;
			/** What the field holds, which the rows match. */
			readonly filter: string;
			readonly frame: WorkbenchQuickInputFrame;
			readonly id: number;
			readonly inputHidden: boolean;
			readonly kind: 'pick';
			/** The rows the filter keeps. */
			readonly rows: readonly WorkbenchQuickPickRow[];
	  }
	| {
			readonly frame: WorkbenchQuickInputFrame;
			readonly id: number;
			readonly ignoreFocusOut: boolean;
			readonly kind: 'input';
			readonly password: boolean;
			readonly prompt: string | undefined;
			readonly validating: boolean;
			readonly validation: WorkbenchQuickInputValidation | null;
			readonly value: string;
	  };

type PickSnapshot = Extract<WorkbenchQuickInputSession, { readonly kind: 'pick' }>;
type InputSnapshot = Extract<WorkbenchQuickInputSession, { readonly kind: 'input' }>;

type ActiveSession =
	| {
			/** Every row the requester gave, before the filter. */
			readonly entries: readonly WorkbenchQuickPickRow[];
			readonly kind: 'pick';
			readonly matchOnDescription: boolean;
			/** The chords whose modifier keys accept once let go, until one is. */
			quickNavigate: readonly WorkbenchQuickNavigateKeys[];
			readonly settle: (itemId: string | null) => void;
			snapshot: PickSnapshot;
	  }
	| {
			controller: AbortController | null;
			readonly kind: 'input';
			readonly settle: (value: string | null) => void;
			snapshot: InputSnapshot;
			readonly validate: WorkbenchQuickInputRequest['validate'];
	  };

const CANCELLED = Object.freeze({ kind: 'cancelled' as const });

/**
 * The quick input's request-driven sessions. One session is current at a time: a new request
 * cancels the current one, and every request settles exactly once, as accepted or cancelled. Events
 * name the session they came from, so one that arrives after its session ended does nothing.
 */
export class WorkbenchQuickInputSessions {
	#active: ActiveSession | null = null;
	readonly #listeners = new Set<() => void>();
	#nextId = 1;

	readonly subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		return () => {
			this.#listeners.delete(listener);
		};
	};

	readonly getSnapshot = (): WorkbenchQuickInputSession | null => this.#active?.snapshot ?? null;

	pick<Value>(
		request: WorkbenchQuickPickRequest<Value>,
	): Promise<WorkbenchQuickInputOutcome<Value>> {
		const { signal } = request;
		if (signal?.aborted === true) return Promise.resolve(CANCELLED);
		return new Promise((resolve) => {
			const id = this.#replace();
			const onAbort = (): void => this.cancel(id);
			signal?.addEventListener('abort', onAbort, { once: true });
			const entries = Object.freeze(request.entries.map(pickRow));
			const items = entries.filter((row) => row.kind === 'item');
			const activeIndex = activationIndex(request.activation ?? 'first', items.length);
			this.#active = {
				entries,
				kind: 'pick',
				matchOnDescription: request.matchOnDescription === true,
				quickNavigate: request.quickNavigate ?? [],
				settle: (itemId) => {
					signal?.removeEventListener('abort', onAbort);
					const entry = request.entries.find(
						(candidate) => candidate.kind === 'item' && candidate.id === itemId,
					);
					resolve(entry?.kind === 'item' ? { kind: 'accepted', value: entry.value } : CANCELLED);
				},
				snapshot: Object.freeze({
					activeId: items[activeIndex]?.id ?? null,
					filter: '',
					frame: request,
					id,
					inputHidden: request.hideInput === true,
					kind: 'pick',
					rows: entries,
				}),
			};
			this.#emit();
		});
	}

	input(request: WorkbenchQuickInputRequest): Promise<WorkbenchQuickInputOutcome<string>> {
		return new Promise((resolve) => {
			const id = this.#replace();
			const active: Extract<ActiveSession, { readonly kind: 'input' }> = {
				controller: null,
				kind: 'input',
				settle: (value) => resolve(value === null ? CANCELLED : { kind: 'accepted', value }),
				snapshot: Object.freeze({
					frame: request,
					id,
					ignoreFocusOut: request.ignoreFocusOut === true,
					kind: 'input',
					password: request.password === true,
					prompt: request.prompt,
					validating: false,
					validation: null,
					value: request.value ?? '',
				}),
				validate: request.validate,
			};
			this.#active = active;
			this.#check(active);
			this.#emit();
		});
	}

	/** Escape, a blur the session does not survive, or the window going away. */
	cancel(sessionId: number): void {
		const active = this.#current(sessionId);
		if (active !== null) this.#end(active, null);
	}

	/** Accepts a pick's item, the active one unless named, or an input's value its check allows. */
	accept(sessionId: number, itemId?: string): boolean {
		const active = this.#current(sessionId);
		if (active === null) return false;
		if (active.kind === 'pick') {
			const chosen = itemId ?? active.snapshot.activeId;
			const listed = active.snapshot.rows.some((row) => row.kind === 'item' && row.id === chosen);
			if (chosen === null || !listed) return false;
			this.#end(active, chosen);
			return true;
		}
		const { validating, validation, value } = active.snapshot;
		if (validating || validation?.severity === 'error') return false;
		this.#end(active, value);
		return true;
	}

	/** Filters a pick's rows by what its field holds; the first item kept becomes active. */
	changeFilter(sessionId: number, filter: string): void {
		const active = this.#current(sessionId);
		if (active?.kind !== 'pick' || active.snapshot.filter === filter) return;
		const rows = shownRows(active.entries, filter, active.matchOnDescription);
		const first = rows.find((row) => row.kind === 'item');
		active.snapshot = Object.freeze({
			...active.snapshot,
			activeId: first?.kind === 'item' ? first.id : null,
			filter,
			rows,
		});
		this.#emit();
	}

	/**
	 * Makes the next or the previous shown item of a pick active, going round; with none active, the
	 * first one, as VS Code's list does.
	 */
	moveActive(sessionId: number, direction: 'next' | 'previous'): void {
		const active = this.#current(sessionId);
		if (active?.kind !== 'pick') return;
		const items = active.snapshot.rows.flatMap((row) => (row.kind === 'item' ? [row.id] : []));
		const index = active.snapshot.activeId === null ? -1 : items.indexOf(active.snapshot.activeId);
		const step = direction === 'next' ? 1 : items.length - 1;
		const activeId = items[index < 0 ? 0 : (index + step) % items.length] ?? null;
		if (activeId === active.snapshot.activeId) return;
		active.snapshot = Object.freeze({ ...active.snapshot, activeId });
		this.#emit();
	}

	/**
	 * VS Code's Quick Open navigate: moves the current pick's active item as `moveActive` does, and
	 * arms quick navigate with the chords that ran it. Answers whether a pick was open.
	 */
	navigate(
		direction: 'next' | 'previous',
		quickNavigate: readonly WorkbenchQuickNavigateKeys[],
	): boolean {
		const active = this.#active;
		if (active?.kind !== 'pick') return false;
		active.quickNavigate = quickNavigate;
		this.moveActive(active.snapshot.id, direction);
		return true;
	}

	/**
	 * A key let go in a pick. While quick navigate is armed, letting go of a modifier key one of its
	 * chords holds ends it and accepts the active item, if any. Shift counts only once no other
	 * modifier key is held, so that a held Control goes on moving back with Shift.
	 */
	keyUp(sessionId: number, release: WorkbenchQuickInputKeyUp): void {
		const active = this.#current(sessionId);
		if (active?.kind !== 'pick') return;
		const othersHeld = release.altKey || release.ctrlKey || release.metaKey;
		const trigger = active.quickNavigate.some((keys) => {
			switch (release.key) {
				case 'Alt':
					return keys.altKey;
				case 'Control':
					return keys.ctrlKey;
				case 'Meta':
					return keys.metaKey;
				case 'Shift':
					return keys.shiftKey && !othersHeld;
				default:
					return false;
			}
		});
		if (!trigger) return;
		active.quickNavigate = [];
		this.accept(sessionId);
	}

	changeValue(sessionId: number, value: string): void {
		const active = this.#current(sessionId);
		if (active?.kind !== 'input' || active.snapshot.value === value) return;
		active.snapshot = Object.freeze({ ...active.snapshot, value });
		this.#check(active);
		this.#emit();
	}

	/** Cancels the current session, as when the Workbench unmounts; later requests still open. */
	close(): void {
		if (this.#active !== null) this.#end(this.#active, null);
	}

	#current(sessionId: number): ActiveSession | null {
		return this.#active?.snapshot.id === sessionId ? this.#active : null;
	}

	#replace(): number {
		if (this.#active !== null) this.#end(this.#active, null);
		const id = this.#nextId;
		this.#nextId += 1;
		return id;
	}

	#end(active: ActiveSession, answer: string | null): void {
		if (active.kind === 'input') active.controller?.abort();
		this.#active = null;
		active.settle(answer);
		this.#emit();
	}

	#check(active: Extract<ActiveSession, { readonly kind: 'input' }>): void {
		active.controller?.abort();
		active.controller = null;
		if (active.validate === undefined) return;
		const controller = new AbortController();
		active.controller = controller;
		active.snapshot = Object.freeze({ ...active.snapshot, validating: true });
		const settle = (validation: WorkbenchQuickInputValidation | null): void => {
			if (this.#active !== active || active.controller !== controller) return;
			active.controller = null;
			active.snapshot = Object.freeze({ ...active.snapshot, validating: false, validation });
			this.#emit();
		};
		active.validate(active.snapshot.value, controller.signal).then(settle, () => settle(null));
	}

	#emit(): void {
		for (const listener of this.#listeners) listener();
	}
}

/** The place of the item a pick makes active first, among `count` items. */
function activationIndex(activation: WorkbenchQuickPickActivation, count: number): number {
	switch (activation) {
		case 'first':
			return 0;
		case 'second':
			return 1;
		case 'last':
			return count - 1;
	}
}

/**
 * The rows a filter keeps: items whose label, or description when the pick matches on it, holds the
 * text typed, and separators only while nothing is typed.
 */
function shownRows(
	rows: readonly WorkbenchQuickPickRow[],
	filter: string,
	matchOnDescription: boolean,
): readonly WorkbenchQuickPickRow[] {
	const needle = filter.trim().toLocaleLowerCase();
	if (needle === '') return rows;
	const matches = (text: string | undefined): boolean =>
		text !== undefined && text.toLocaleLowerCase().includes(needle);
	return Object.freeze(
		rows.filter(
			(row) =>
				row.kind === 'item' &&
				(matches(row.label) || (matchOnDescription && matches(row.description))),
		),
	);
}

function pickRow<Value>(entry: WorkbenchQuickPickEntry<Value>): WorkbenchQuickPickRow {
	return entry.kind === 'separator'
		? Object.freeze({ kind: 'separator', label: entry.label })
		: Object.freeze({
				description: entry.description,
				icon: entry.icon,
				id: entry.id,
				italic: entry.italic === true,
				kind: 'item',
				label: entry.label,
			});
}
