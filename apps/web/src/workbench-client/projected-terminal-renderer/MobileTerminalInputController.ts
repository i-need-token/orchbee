import type {
	ProjectedTerminal,
	ProjectedTerminalInjectedInput,
	ProjectedTerminalInjectedKeyModifiers,
} from '@orchbee/terminal';

import {
	clearMobileTerminalModifiers,
	consumeMobileTerminalInput,
	createMobileTerminalInputState,
	replaceMobileTerminalInputReadiness,
	setMobileTerminalInputFocus,
	toggleMobileTerminalModifier,
	type MobileTerminalInputSnapshot,
	type MobileTerminalInputState,
	type MobileTerminalModifier,
} from './mobile-terminal-input-state';

export type MobileTerminalKey =
	| 'ArrowDown'
	| 'ArrowLeft'
	| 'ArrowRight'
	| 'ArrowUp'
	| 'Escape'
	| 'Tab';

export type MobileTerminalInputDriver = Pick<
	ProjectedTerminal,
	| 'blur'
	| 'focusMobileInput'
	| 'injectInput'
	| 'injectedInputReadiness'
	| 'onInjectedInputReadinessChange'
	| 'textarea'
>;

export interface MobileTerminalInputSubscription {
	dispose(): void;
}

/** Owns one attachment-local mobile input latch and its native capture boundary. */
export class MobileTerminalInputController {
	readonly #driver: MobileTerminalInputDriver;
	readonly #listeners = new Set<() => void>();
	readonly #root: HTMLElement;
	#disposed = false;
	#readinessSubscription: MobileTerminalInputSubscription;
	#state: MobileTerminalInputState;

	constructor(root: HTMLElement, driver: MobileTerminalInputDriver) {
		this.#root = root;
		this.#driver = driver;
		this.#state = createMobileTerminalInputState(
			driver.injectedInputReadiness,
			driver.textarea?.ownerDocument.activeElement === driver.textarea,
		);
		root.addEventListener('keydown', this.#handleKeyDown, { capture: true });
		root.addEventListener('beforeinput', this.#handleBeforeInput, { capture: true });
		root.addEventListener('compositionstart', this.#handleCompositionStart, { capture: true });
		root.addEventListener('input', this.#handleInput, { capture: true });
		root.addEventListener('paste', this.#handlePaste, { capture: true });
		driver.textarea?.addEventListener('focus', this.#handleFocus);
		driver.textarea?.addEventListener('blur', this.#handleBlur);
		this.#readinessSubscription = driver.onInjectedInputReadinessChange(() => {
			this.#replaceState(
				replaceMobileTerminalInputReadiness(this.#state, driver.injectedInputReadiness),
			);
		});
	}

	get state(): MobileTerminalInputState {
		return this.#state;
	}

	subscribe(listener: () => void): MobileTerminalInputSubscription {
		if (!this.#disposed) this.#listeners.add(listener);
		return { dispose: () => this.#listeners.delete(listener) };
	}

	toggleModifier(modifier: MobileTerminalModifier): void {
		if (this.#disposed) return;
		this.#replaceState(toggleMobileTerminalModifier(this.#state, modifier));
	}

	sendKey(key: MobileTerminalKey): boolean {
		if (this.#disposed) return false;
		const snapshot = this.#consume();
		if (snapshot === null) return false;
		this.#inject(keyInput(key, snapshot), snapshot.revision);
		return true;
	}

	toggleKeyboard(): boolean {
		if (this.#disposed) return false;
		const textarea = this.#driver.textarea;
		if (textarea === undefined) return false;
		if (textarea.ownerDocument.activeElement === textarea) {
			this.#driver.blur();
			return true;
		}
		if (this.#state.readiness.kind !== 'ready') return false;
		return this.#driver.focusMobileInput();
	}

	clearModifiers(): void {
		if (this.#disposed) return;
		this.#replaceState(clearMobileTerminalModifiers(this.#state));
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#root.removeEventListener('keydown', this.#handleKeyDown, { capture: true });
		this.#root.removeEventListener('beforeinput', this.#handleBeforeInput, { capture: true });
		this.#root.removeEventListener('compositionstart', this.#handleCompositionStart, {
			capture: true,
		});
		this.#root.removeEventListener('input', this.#handleInput, { capture: true });
		this.#root.removeEventListener('paste', this.#handlePaste, { capture: true });
		this.#driver.textarea?.removeEventListener('focus', this.#handleFocus);
		this.#driver.textarea?.removeEventListener('blur', this.#handleBlur);
		this.#readinessSubscription.dispose();
		this.#listeners.clear();
		this.#state = createMobileTerminalInputState();
	}

	readonly #handleFocus = (): void => {
		this.#replaceState(setMobileTerminalInputFocus(this.#state, true));
	};

	readonly #handleBlur = (): void => {
		this.#replaceState(setMobileTerminalInputFocus(this.#state, false));
	};

	readonly #handleCompositionStart = (event: Event): void => {
		if (event.target === this.#driver.textarea) this.clearModifiers();
	};

	readonly #handlePaste = (event: Event): void => {
		if (event.target === this.#driver.textarea) this.clearModifiers();
	};

	readonly #handleInput = (event: Event): void => {
		if (event.target === this.#driver.textarea) this.clearModifiers();
	};

	readonly #handleBeforeInput = (event: InputEvent): void => {
		if (event.target !== this.#driver.textarea || !this.#hasArmedModifier()) return;
		if (
			event.isComposing ||
			event.inputType === 'insertCompositionText' ||
			event.inputType === 'insertFromComposition' ||
			event.inputType === 'insertFromPaste'
		) {
			this.clearModifiers();
			return;
		}
		const nativeKey = asciiNativeKey(event.data ?? '');
		if (nativeKey === null) {
			if (event.data !== null) this.clearModifiers();
			return;
		}
		const snapshot = this.#consume();
		if (snapshot === null) return;
		event.preventDefault();
		event.stopImmediatePropagation();
		this.#inject(
			{
				code: nativeKey.code,
				key: nativeKey.key,
				kind: 'key',
				modifiers: injectedModifiers(snapshot, { shift: nativeKey.shift }),
				repeat: false,
			},
			snapshot.revision,
		);
	};

	readonly #handleKeyDown = (event: KeyboardEvent): void => {
		if (event.target !== this.#driver.textarea || !this.#hasArmedModifier()) return;
		if (event.isComposing || event.keyCode === 229) {
			this.clearModifiers();
			return;
		}
		if (!eligiblePhysicalKey(event)) return;
		const snapshot = this.#consume();
		if (snapshot === null) return;
		event.preventDefault();
		event.stopImmediatePropagation();
		this.#inject(
			{
				code: event.code,
				key: event.key,
				kind: 'key',
				modifiers: injectedModifiers(snapshot, {
					alt: event.altKey,
					altGraph: event.getModifierState?.('AltGraph') ?? false,
					capsLock: event.getModifierState?.('CapsLock') ?? false,
					control: event.ctrlKey,
					meta: event.metaKey,
					numLock: event.getModifierState?.('NumLock') ?? false,
					shift: event.shiftKey,
				}),
				repeat: event.repeat,
			},
			snapshot.revision,
		);
	};

	#hasArmedModifier(): boolean {
		return this.#state.alt || this.#state.control;
	}

	#consume(): MobileTerminalInputSnapshot | null {
		const consumption = consumeMobileTerminalInput(this.#state);
		this.#replaceState(consumption.state);
		return consumption.snapshot;
	}

	#inject(input: ProjectedTerminalInjectedInput, revision: string): void {
		void this.#driver.injectInput(input, revision).catch(() => undefined);
	}

	#replaceState(state: MobileTerminalInputState): void {
		if (state === this.#state) return;
		this.#state = state;
		for (const listener of this.#listeners) listener();
	}
}

function keyInput(
	key: MobileTerminalKey,
	snapshot: MobileTerminalInputSnapshot,
): ProjectedTerminalInjectedInput {
	return {
		code: key,
		key,
		kind: 'key',
		modifiers: injectedModifiers(snapshot),
		repeat: false,
	};
}

interface NativeModifierFacts {
	readonly alt?: boolean;
	readonly altGraph?: boolean;
	readonly capsLock?: boolean;
	readonly control?: boolean;
	readonly meta?: boolean;
	readonly numLock?: boolean;
	readonly shift?: boolean;
}

function injectedModifiers(
	snapshot: MobileTerminalInputSnapshot,
	native: NativeModifierFacts = {},
): ProjectedTerminalInjectedKeyModifiers {
	return {
		alt: snapshot.alt || native.alt === true,
		altGraph: native.altGraph === true,
		altLeft: snapshot.alt || native.alt === true,
		altRight: false,
		capsLock: native.capsLock === true,
		control: snapshot.control || native.control === true,
		controlRight: false,
		meta: native.meta === true,
		metaRight: false,
		numLock: native.numLock === true,
		shift: native.shift === true,
		shiftRight: false,
	};
}

function eligiblePhysicalKey(event: KeyboardEvent): boolean {
	return (
		event.code !== '' &&
		!event.code.startsWith('Alt') &&
		!event.code.startsWith('Control') &&
		!event.code.startsWith('Meta') &&
		!event.code.startsWith('Shift') &&
		!['Alt', 'Control', 'Dead', 'Meta', 'Process', 'Shift', 'Unidentified'].includes(event.key)
	);
}

interface AsciiNativeKey {
	readonly code: string;
	readonly key: string;
	readonly shift: boolean;
}

function asciiNativeKey(text: string): AsciiNativeKey | null {
	if (text.length !== 1) return null;
	const codepoint = text.codePointAt(0);
	if (codepoint === undefined || codepoint < 0x20 || codepoint > 0x7e) return null;
	if (/^[a-z]$/.test(text)) return { code: `Key${text.toUpperCase()}`, key: text, shift: false };
	if (/^[A-Z]$/.test(text)) return { code: `Key${text}`, key: text, shift: true };
	if (/^[0-9]$/.test(text)) return { code: `Digit${text}`, key: text, shift: false };
	if (text === ' ') return { code: 'Space', key: text, shift: false };
	const punctuation = asciiPunctuation.get(text);
	return punctuation === undefined ? null : { ...punctuation, key: text };
}

const asciiPunctuation: ReadonlyMap<string, Omit<AsciiNativeKey, 'key'>> = new Map([
	['`', { code: 'Backquote', shift: false }],
	['~', { code: 'Backquote', shift: true }],
	['-', { code: 'Minus', shift: false }],
	['_', { code: 'Minus', shift: true }],
	['=', { code: 'Equal', shift: false }],
	['+', { code: 'Equal', shift: true }],
	['[', { code: 'BracketLeft', shift: false }],
	['{', { code: 'BracketLeft', shift: true }],
	[']', { code: 'BracketRight', shift: false }],
	['}', { code: 'BracketRight', shift: true }],
	['\\', { code: 'Backslash', shift: false }],
	['|', { code: 'Backslash', shift: true }],
	[';', { code: 'Semicolon', shift: false }],
	[':', { code: 'Semicolon', shift: true }],
	["'", { code: 'Quote', shift: false }],
	['"', { code: 'Quote', shift: true }],
	[',', { code: 'Comma', shift: false }],
	['<', { code: 'Comma', shift: true }],
	['.', { code: 'Period', shift: false }],
	['>', { code: 'Period', shift: true }],
	['/', { code: 'Slash', shift: false }],
	['?', { code: 'Slash', shift: true }],
	['!', { code: 'Digit1', shift: true }],
	['@', { code: 'Digit2', shift: true }],
	['#', { code: 'Digit3', shift: true }],
	['$', { code: 'Digit4', shift: true }],
	['%', { code: 'Digit5', shift: true }],
	['^', { code: 'Digit6', shift: true }],
	['&', { code: 'Digit7', shift: true }],
	['*', { code: 'Digit8', shift: true }],
	['(', { code: 'Digit9', shift: true }],
	[')', { code: 'Digit0', shift: true }],
]);
