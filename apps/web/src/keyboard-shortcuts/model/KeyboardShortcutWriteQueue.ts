/** Serialize the shortcut overrides' synchronous writes, retaining only the latest pending value. */
export class KeyboardShortcutWriteQueue {
	#disposed = false;
	#pending: (() => void) | undefined;
	#writing = false;

	get pending(): boolean {
		return this.#writing || this.#pending !== undefined;
	}

	write(operation: () => void): void {
		if (this.#disposed) return;
		this.#pending = operation;
		if (this.#writing) return;
		this.#writing = true;
		try {
			while (this.#pending !== undefined && !this.#disposed) {
				const next = this.#pending;
				this.#pending = undefined;
				next();
			}
		} finally {
			this.#writing = false;
		}
	}

	dispose(): void {
		this.#disposed = true;
		this.#pending = undefined;
	}
}
