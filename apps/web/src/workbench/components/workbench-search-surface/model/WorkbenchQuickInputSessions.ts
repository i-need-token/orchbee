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

/** What every session shows around its rows or its field. */
interface WorkbenchQuickInputFrame {
	/** Shown while the requester is still working, such as loading a later step's items. */
	readonly busy?: boolean | undefined;
	readonly placeholder?: string | undefined;
	readonly step?: { readonly current: number; readonly total: number } | undefined;
	readonly title?: string | undefined;
}

export interface WorkbenchQuickPickRequest<Value> extends WorkbenchQuickInputFrame {
	readonly entries: readonly WorkbenchQuickPickEntry<Value>[];
	/** Filters on the items' descriptions as well as their labels. */
	readonly matchOnDescription?: boolean | undefined;
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

export type WorkbenchQuickPickRow =
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
			readonly frame: WorkbenchQuickInputFrame;
			readonly id: number;
			readonly kind: 'pick';
			readonly matchOnDescription: boolean;
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
			readonly kind: 'pick';
			readonly settle: (itemId: string | null) => void;
			readonly snapshot: PickSnapshot;
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
			this.#active = {
				kind: 'pick',
				settle: (itemId) => {
					signal?.removeEventListener('abort', onAbort);
					const entry = request.entries.find(
						(candidate) => candidate.kind === 'item' && candidate.id === itemId,
					);
					resolve(entry?.kind === 'item' ? { kind: 'accepted', value: entry.value } : CANCELLED);
				},
				snapshot: Object.freeze({
					frame: request,
					id,
					kind: 'pick',
					matchOnDescription: request.matchOnDescription === true,
					rows: Object.freeze(request.entries.map(pickRow)),
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

	/** Accepts a pick's item, or an input's value once its check allows it. */
	accept(sessionId: number, itemId?: string): boolean {
		const active = this.#current(sessionId);
		if (active === null) return false;
		if (active.kind === 'pick') {
			const listed = active.snapshot.rows.some((row) => row.kind === 'item' && row.id === itemId);
			if (itemId === undefined || !listed) return false;
			this.#end(active, itemId);
			return true;
		}
		const { validating, validation, value } = active.snapshot;
		if (validating || validation?.severity === 'error') return false;
		this.#end(active, value);
		return true;
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
