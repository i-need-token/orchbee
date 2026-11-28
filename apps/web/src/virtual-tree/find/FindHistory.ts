/** How many patterns a find remembers, as VS Code's history input box keeps. */
const HISTORY_LIMIT = 100;

/**
 * The patterns a tree's find remembers, oldest first, and the place Up and Down have walked to
 * among them, as VS Code's history input box keeps them: a pattern remembered again moves to the
 * end, the oldest fall away past 100, and remembering one puts the walk past the newest, where
 * nothing is recalled.
 */
export class FindHistory {
	private items: string[] = [];
	/** The walk's place: -1 before the oldest pattern and `items.length` past the newest. */
	private index = 0;

	/**
	 * Remembers `value` unless it is empty or, without `always`, the pattern the walk stands on, or
	 * the newest one while it stands nowhere.
	 */
	add(value: string, always = false): void {
		if (value === '' || (!always && value === this.currentValue())) return;
		const existing = this.items.indexOf(value);
		if (existing >= 0) this.items.splice(existing, 1);
		this.items.push(value);
		if (this.items.length > HISTORY_LIMIT) this.items.splice(0, this.items.length - HISTORY_LIMIT);
		this.index = this.items.length;
	}

	/** Puts the walk past the newest pattern, where a find that opens starts it. */
	restart(): void {
		this.index = this.items.length;
	}

	/**
	 * Up from an input showing `value`: the pattern before the one the walk stands on, skipping one
	 * equal to `value`, or the oldest; null when there is none. A value not yet remembered is
	 * remembered first.
	 */
	previous(value: string): string | null {
		if (!this.items.includes(value)) this.add(value);
		let previous = this.stepBack() ?? this.first();
		if (previous !== null && previous === value) previous = this.stepBack() ?? this.first();
		return previous;
	}

	/**
	 * Down from an input showing `value`: the pattern after the one the walk stands on, skipping one
	 * equal to `value`, or the empty string past the newest. A value not yet remembered is remembered
	 * first.
	 */
	next(value: string): string {
		if (!this.items.includes(value)) this.add(value);
		let next = this.stepForward();
		if (next !== null && next === value) next = this.stepForward();
		return next ?? '';
	}

	/** Whether the walk stands on the newest pattern or nowhere, where Down leaves the input. */
	get atEnd(): boolean {
		const current = this.current();
		return current === null || this.index >= this.items.length - 1;
	}

	private current(): string | null {
		return this.items[this.index] ?? null;
	}

	/** The pattern the walk stands on, or else the newest one, leaving the walk where it was. */
	private currentValue(): string | null {
		return this.current() ?? this.items.at(-1) ?? null;
	}

	private stepBack(): string | null {
		if (this.index === 0 && this.current() !== null) return null;
		this.index = Math.max(this.index - 1, -1);
		return this.current();
	}

	private stepForward(): string | null {
		this.index = Math.min(this.index + 1, this.items.length);
		return this.current();
	}

	private first(): string | null {
		this.index = 0;
		return this.current();
	}
}
