import { SCM_COMMIT_INPUT_MIN_HEIGHT } from '../constants/scm-rows';
import type { ScmPostCommit } from './scm-action-button';

/** Why the message changed other than by typing, which decides where the cursor lands. */
export type ScmCommitInputChange = 'history-next' | 'history-previous' | 'set';

/** A selection as Monaco's `ISelection` states it, kept while the input's editor is away. */
export interface ScmCommitInputSelection {
	readonly positionColumn: number;
	readonly positionLineNumber: number;
	readonly selectionStartColumn: number;
	readonly selectionStartLineNumber: number;
}

/** VS Code's SCM input history holds 100 messages. */
const HISTORY_CAPACITY = 100;

/**
 * One repository's commit input for the run, whether or not its row shows: the message, VS Code's
 * message history, the selections and height its editor last had, and the post-commit command the
 * action button remembers. The history is VS Code's `HistoryNavigator2`: unique messages, oldest
 * first, whose last entry is the draft and where the cursor rests until Up walks back.
 */
export class ScmCommitInput {
	/** The height of the editor's content, which its row follows. */
	height = SCM_COMMIT_INPUT_MIN_HEIGHT;
	/** VS Code's stored post-commit command: set by a press of the button, cleared after its commit. */
	remembered: ScmPostCommit | undefined = undefined;
	selections: readonly ScmCommitInputSelection[] | null = null;
	readonly #history: string[] = [''];
	#cursor = 0;
	readonly #listeners = new Set<(change: ScmCommitInputChange) => void>();
	#value = '';

	get value(): string {
		return this.#value;
	}

	/** Hears each change made other than by typing, which the shown editor applies. */
	subscribe(listener: (change: ScmCommitInputChange) => void): () => void {
		this.#listeners.add(listener);
		return () => {
			this.#listeners.delete(listener);
		};
	}

	/** What typing leaves in the input, which stays out of the history until something replaces it. */
	type(value: string): void {
		this.#value = value;
	}

	/** A message set other than by typing, such as the empty one after a commit, which the history keeps. */
	set(value: string): void {
		if (value === this.#value) return;
		this.#replaceLast(this.#value);
		this.#add(value);
		this.#change(value, 'set');
	}

	/** VS Code's Clear Input on Escape: empties the input as typing would, leaving the history alone. */
	clear(): void {
		this.#change('', 'set');
	}

	/** VS Code's Previous Commit Message, which keeps the draft as the history's last entry. */
	showPrevious(): void {
		if (this.#atEnd() || !this.#history.includes(this.#value)) {
			this.#replaceLast(this.#value);
			this.#cursor = this.#history.length - 1;
		}
		if (this.#cursor > 0) this.#cursor -= 1;
		this.#change(this.#history[this.#cursor] ?? '', 'history-previous');
	}

	/** VS Code's Next Commit Message, which stops at the draft. */
	showNext(): void {
		if (this.#atEnd()) return;
		if (!this.#history.includes(this.#value)) {
			this.#replaceLast(this.#value);
			this.#cursor = this.#history.length - 1;
		}
		if (this.#cursor < this.#history.length - 1) this.#cursor += 1;
		this.#change(this.#history[this.#cursor] ?? '', 'history-next');
	}

	#change(value: string, change: ScmCommitInputChange): void {
		if (value === this.#value) return;
		this.#value = value;
		for (const listener of this.#listeners) listener(change);
	}

	#atEnd(): boolean {
		return this.#cursor === this.#history.length - 1;
	}

	#add(value: string): void {
		this.#history.push(value);
		this.#removeEarlier(value);
		if (this.#history.length > HISTORY_CAPACITY) this.#history.shift();
		this.#cursor = this.#history.length - 1;
	}

	#replaceLast(value: string): void {
		const last = this.#history.length - 1;
		if (this.#history[last] === value) return;
		const current = this.#cursor === last ? null : (this.#history[this.#cursor] ?? null);
		this.#history[last] = value;
		this.#removeEarlier(value);
		const index = current === null ? -1 : this.#history.indexOf(current);
		this.#cursor = index >= 0 ? index : this.#history.length - 1;
	}

	/** Drops the entries before the last that equal `value`, as the history keeps messages unique. */
	#removeEarlier(value: string): void {
		for (let index = this.#history.length - 2; index >= 0; index -= 1) {
			if (this.#history[index] === value) this.#history.splice(index, 1);
		}
	}
}
