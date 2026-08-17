export type DesktopSecureKeyboardEntryState = 'active' | 'inactive' | 'unavailable' | 'unsupported';

export interface DesktopSecureKeyboardEntryAdapter {
	isEnabled(): boolean;
	setEnabled(enabled: boolean): void;
}

export class DesktopSecureKeyboardEntry {
	readonly #adapter: DesktopSecureKeyboardEntryAdapter;
	#applicationActive: boolean;
	#disposed = false;
	#state: DesktopSecureKeyboardEntryState;

	constructor(
		platform: NodeJS.Platform,
		adapter: DesktopSecureKeyboardEntryAdapter,
		applicationActive = platform === 'darwin',
	) {
		this.#adapter = adapter;
		this.#applicationActive = applicationActive;
		if (platform !== 'darwin') {
			this.#state = 'unsupported';
			return;
		}
		try {
			this.#state = adapter.isEnabled() ? 'active' : 'inactive';
			if (!this.#applicationActive && this.#state === 'active') this.release();
		} catch {
			this.#state = 'unavailable';
			this.#disableAfterFailure();
		}
	}

	getState(): DesktopSecureKeyboardEntryState {
		return this.#disposed ? 'unavailable' : this.#state;
	}

	activate(): void {
		if (this.#disposed || this.#state === 'unsupported') return;
		this.#applicationActive = true;
	}

	deactivate(): void {
		this.#applicationActive = false;
		this.release();
	}

	toggle(): boolean {
		if (
			this.#disposed ||
			!this.#applicationActive ||
			this.#state === 'unavailable' ||
			this.#state === 'unsupported'
		) {
			return false;
		}
		return this.#apply(this.#state !== 'active');
	}

	release(): void {
		if (this.#disposed || this.#state !== 'active') return;
		this.#apply(false);
	}

	dispose(): void {
		if (this.#disposed) return;
		this.release();
		this.#disposed = true;
	}

	#apply(enabled: boolean): boolean {
		try {
			this.#adapter.setEnabled(enabled);
			if (this.#adapter.isEnabled() !== enabled) {
				this.#state = 'unavailable';
				this.#disableAfterFailure();
				return false;
			}
			this.#state = enabled ? 'active' : 'inactive';
			return true;
		} catch {
			this.#state = 'unavailable';
			this.#disableAfterFailure();
			return false;
		}
	}

	#disableAfterFailure(): void {
		try {
			this.#adapter.setEnabled(false);
		} catch {
			// A best-effort fail-closed release must not obscure the original platform failure.
		}
	}
}
