import type { TerminalHint, TerminalHintChoiceResult } from '../terminal-hints';

interface PresentedTerminalHint {
	readonly hint: TerminalHint;
	readonly label: string;
}

interface TerminalHintSessionOptions {
	readonly canvas: HTMLCanvasElement;
	readonly externalSignal: AbortSignal;
	readonly loadingText?: string | undefined;
	readonly root: HTMLElement;
}

interface TerminalHintPresentation {
	readonly cellHeight: number;
	readonly cellWidth: number;
	readonly fontFamily: string;
	readonly fontSize: number;
	readonly hints: readonly TerminalHint[];
	readonly labels: readonly string[];
	readonly visibleStartRow: bigint;
}

/** One bounded keyboard-owned hint overlay and its cancellation lifetime. */
export class TerminalHintSession {
	readonly #abortController = new AbortController();
	readonly #completion: Promise<TerminalHintChoiceResult>;
	readonly #externalSignal: AbortSignal;
	readonly #handleExternalAbort: () => void;
	readonly #overlay: HTMLDivElement;
	#active = true;
	#choices: readonly PresentedTerminalHint[] = [];
	#prefix = '';
	#resolveCompletion: (result: TerminalHintChoiceResult) => void = () => undefined;

	constructor({ canvas, externalSignal, loadingText, root }: TerminalHintSessionOptions) {
		this.#externalSignal = externalSignal;
		this.#completion = new Promise((resolve) => {
			this.#resolveCompletion = resolve;
		});
		const overlay = root.ownerDocument.createElement('div');
		overlay.dataset['projectedTerminalHints'] = 'true';
		overlay.dataset['state'] = 'loading';
		overlay.setAttribute('aria-hidden', 'true');
		const rootBounds = root.getBoundingClientRect();
		const canvasBounds = canvas.getBoundingClientRect();
		overlay.style.position = 'absolute';
		overlay.style.left = `${canvasBounds.left - rootBounds.left}px`;
		overlay.style.top = `${canvasBounds.top - rootBounds.top}px`;
		overlay.style.width = `${canvasBounds.width}px`;
		overlay.style.height = `${canvasBounds.height}px`;
		if (loadingText !== undefined) {
			const loading = root.ownerDocument.createElement('span');
			loading.dataset['projectedTerminalHintsLoading'] = 'true';
			loading.textContent = loadingText;
			overlay.appendChild(loading);
		}
		root.appendChild(overlay);
		this.#overlay = overlay;
		this.#handleExternalAbort = () => this.cancel('cancelled');
		if (externalSignal.aborted) this.cancel('cancelled');
		else externalSignal.addEventListener('abort', this.#handleExternalAbort, { once: true });
	}

	get active(): boolean {
		return this.#active;
	}

	get completion(): Promise<TerminalHintChoiceResult> {
		return this.#completion;
	}

	get signal(): AbortSignal {
		return this.#abortController.signal;
	}

	present({
		cellHeight,
		cellWidth,
		fontFamily,
		fontSize,
		hints,
		labels,
		visibleStartRow,
	}: TerminalHintPresentation): boolean {
		if (!this.#active || hints.length === 0 || hints.length !== labels.length) return false;
		this.#overlay.replaceChildren();
		this.#overlay.dataset['state'] = 'active';
		this.#overlay.dataset['hintCount'] = String(hints.length);
		this.#choices = hints.flatMap((hint, index) => {
			const label = labels[index];
			if (label === undefined) return [];
			const element = this.#overlay.ownerDocument.createElement('span');
			element.dataset['projectedTerminalHint'] = 'true';
			element.dataset['projectedTerminalHintKind'] = hint.kind;
			element.dataset['projectedTerminalHintLabel'] = label;
			element.style.left = `${hint.range.start.x * cellWidth}px`;
			element.style.top = `${(hint.range.start.y - Number(visibleStartRow)) * cellHeight}px`;
			element.style.width = `${Math.max(cellWidth, label.length * cellWidth)}px`;
			element.style.height = `${cellHeight}px`;
			element.style.fontFamily = fontFamily;
			element.style.fontSize = `${Math.min(fontSize, cellHeight)}px`;
			element.textContent = label;
			this.#overlay.appendChild(element);
			return [{ hint, label }];
		});
		return this.#choices.length > 0;
	}

	finish(result: { readonly kind: 'empty' | 'unavailable' }): void {
		this.#complete(result);
	}

	cancel(kind: 'cancelled' | 'stale' = 'cancelled'): void {
		this.#complete({ kind });
	}

	handleKeyDown(event: KeyboardEvent): boolean {
		if (!this.#active) return false;
		if (event.isComposing || event.keyCode === 229) {
			this.cancel('cancelled');
			return false;
		}
		this.#own(event);
		if (event.key === 'Escape') {
			this.cancel('cancelled');
			return true;
		}
		if (this.#choices.length === 0 || event.repeat) return true;
		if (event.key === 'Backspace') {
			this.#prefix = this.#prefix.slice(0, -1);
			this.#updateChoices();
			return true;
		}
		const visible = this.#choices.filter(({ label }) => label.startsWith(this.#prefix));
		if ((event.key === 'Enter' || event.key === ' ') && visible.length === 1) {
			const choice = visible[0];
			if (choice !== undefined) this.#complete({ hint: choice.hint, kind: 'selected' });
			return true;
		}
		if (event.altKey || event.ctrlKey || event.metaKey || event.key.length !== 1) return true;
		const nextPrefix = `${this.#prefix}${event.key.toLowerCase()}`;
		const matches = this.#choices.filter(({ label }) => label.startsWith(nextPrefix));
		if (matches.length === 0) return true;
		const selected = matches.find(({ label }) => label === nextPrefix);
		if (selected !== undefined) {
			this.#complete({ hint: selected.hint, kind: 'selected' });
			return true;
		}
		this.#prefix = nextPrefix;
		this.#updateChoices();
		return true;
	}

	handleKeyUp(event: KeyboardEvent): boolean {
		if (!this.#active) return false;
		this.#own(event);
		return true;
	}

	#updateChoices(): void {
		this.#overlay.dataset['prefix'] = this.#prefix;
		for (const element of this.#overlay.querySelectorAll<HTMLElement>(
			'[data-projected-terminal-hint="true"]',
		)) {
			const label = element.dataset['projectedTerminalHintLabel'];
			const matches = label?.startsWith(this.#prefix) === true;
			element.hidden = !matches;
			if (matches && label !== undefined)
				element.textContent = label.slice(this.#prefix.length) || '•';
		}
	}

	#complete(result: TerminalHintChoiceResult): void {
		if (!this.#active) return;
		this.#active = false;
		this.#externalSignal.removeEventListener('abort', this.#handleExternalAbort);
		if (!this.#abortController.signal.aborted) this.#abortController.abort(result.kind);
		this.#overlay.remove();
		this.#resolveCompletion(result);
	}

	#own(event: KeyboardEvent): void {
		event.preventDefault();
		event.stopPropagation();
	}
}
