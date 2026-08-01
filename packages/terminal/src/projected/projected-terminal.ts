import {
	KeyAction,
	CanonicalUint64TextSchema,
	MouseAction,
	MouseButton,
	PROTOCOL_LIMITS,
	TerminalKeyCode,
	type AttachmentId,
	type CanonicalUint64Text,
	type CreationId,
	type FocusInput,
	type FrameApplied,
	type HostEpoch,
	type KeyInput,
	type MouseInput,
	type NeedSnapshot,
	type PasteInput,
	type ProjectionDemand,
	type ResizeTerminalV1_1,
	type SetProjectionDemand,
	type TerminalExited,
	type TerminalId,
	type TerminalKeyCodeValue,
	type TerminalModifiers,
	type TextInput,
} from '@orchbee/terminal-protocol';

import { EventEmitter, type IEvent } from '../EventEmitter';
import { buildPreeditState, shouldSuppressComposingControlInput } from '../input';
import {
	type RendererFrameTelemetry,
	type RendererHealthEvent,
	WebGPURenderer,
} from '../rendering';
import type { RendererOptions } from '../types';
import {
	PackedTerminalStore,
	projectedCellDisplayWidth,
	type ProjectedTerminalMetadata,
} from './packed-terminal-store';

const terminalKeyCodes: ReadonlyMap<string, TerminalKeyCodeValue> = new Map(
	TerminalKeyCode.map((code) => [code, code]),
);
const textEncoder = new TextEncoder();
const INPUT_DEDUPLICATION_MS = 100;
const BRACKETED_PASTE_END = '\u001b[201~';
const MAX_PTY_PIXEL_DIMENSION = 0xffff;

export type ProjectedTerminalCommand =
	| { readonly kind: 'focus-input'; readonly payload: FocusInput }
	| { readonly kind: 'frame-applied'; readonly payload: FrameApplied }
	| { readonly kind: 'key-input'; readonly payload: KeyInput }
	| { readonly kind: 'mouse-input'; readonly payload: MouseInput }
	| { readonly kind: 'need-snapshot'; readonly payload: NeedSnapshot }
	| { readonly kind: 'paste-input'; readonly payload: PasteInput }
	| { readonly kind: 'resize-terminal'; readonly payload: ResizeTerminalV1_1 }
	| { readonly kind: 'set-projection-demand'; readonly payload: SetProjectionDemand }
	| { readonly kind: 'text-input'; readonly payload: TextInput };

export interface ProjectedTerminalCommandPort {
	send(command: ProjectedTerminalCommand): Promise<void> | void;
}

export interface ProjectedTerminalLabels {
	/** Localized label for the hidden keyboard/IME input. */
	readonly input: string;
	/** Localized label for the visible-viewport accessibility mirror. */
	readonly screen: string;
}

export interface ProjectedPasteReview {
	readonly bracketed: boolean;
	readonly embeddedBracketTerminator: boolean;
	readonly multiline: boolean;
	readonly replacedControlCharacters: number;
	readonly source: ProjectedPasteSource;
	readonly text: string;
}

export type ProjectedPasteSource = 'api' | 'beforeinput' | 'clipboard' | 'drop';

export type ProjectedPasteResult =
	| { readonly kind: 'blocked'; readonly review: ProjectedPasteReview }
	| { readonly kind: 'empty' }
	| { readonly kind: 'sent'; readonly review: ProjectedPasteReview };

export interface ProjectedTerminalOptions {
	readonly attachmentId: AttachmentId;
	readonly autoActivate?: boolean;
	readonly canResize?: boolean;
	readonly creationId: CreationId;
	readonly hostEpoch: HostEpoch;
	readonly inputEnabled?: boolean;
	readonly labels: ProjectedTerminalLabels;
	readonly port: ProjectedTerminalCommandPort;
	readonly renderer?: RendererOptions;
	readonly reviewPaste?: (review: ProjectedPasteReview) => boolean | Promise<boolean>;
	readonly terminalId: TerminalId;
}

export interface ProjectedTerminalErrorEvent {
	readonly error: unknown;
	readonly operation: 'command' | 'frame' | 'open' | 'render' | 'resize';
}

export interface ProjectedTerminalFrameTelemetry extends RendererFrameTelemetry {
	readonly revision: CanonicalUint64Text;
}

export type ProjectedFrameResult = 'committed' | 'disposed' | 'duplicate' | 'snapshot-required';

export const ProjectedTerminalPhase = {
	constructed: 'constructed',
	disposed: 'disposed',
	open: 'open',
	opening: 'opening',
} as const;

export type ProjectedTerminalPhase =
	(typeof ProjectedTerminalPhase)[keyof typeof ProjectedTerminalPhase];

interface KeyFacts {
	readonly composing: boolean;
	readonly consumedModifiers: TerminalModifiers;
	readonly key: TerminalKeyCodeValue;
	readonly modifiers: TerminalModifiers;
	readonly text: string;
	readonly unshiftedCodepoint: number;
}

interface SentInput {
	readonly text: string;
	readonly time: number;
}

interface ResizeGeometry {
	readonly cellPixelHeight: number;
	readonly cellPixelWidth: number;
	readonly columns: number;
	readonly rows: number;
}

type ScheduledFrame =
	| { readonly kind: 'animation-frame'; readonly value: number }
	| { readonly kind: 'timeout'; readonly value: ReturnType<typeof setTimeout> };

export class ProjectedTerminal {
	readonly #accessibilityRows: HTMLDivElement[] = [];
	readonly #attachmentId: AttachmentId;
	readonly #creationId: CreationId;
	readonly #errorEmitter = new EventEmitter<ProjectedTerminalErrorEvent>();
	readonly #exitEmitter = new EventEmitter<TerminalExited>();
	readonly #frameTelemetryEmitter = new EventEmitter<ProjectedTerminalFrameTelemetry>();
	readonly #healthEmitter = new EventEmitter<RendererHealthEvent>();
	readonly #hostEpoch: HostEpoch;
	readonly #metadataEmitter = new EventEmitter<ProjectedTerminalMetadata>();
	readonly #options: ProjectedTerminalOptions;
	readonly #pressedKeys = new Map<string, KeyFacts>();
	readonly #pressedModifierCodes = new Set<string>();
	readonly #store: PackedTerminalStore;
	readonly #terminalId: TerminalId;
	#accessibilityRoot: HTMLDivElement | undefined;
	#canvas: HTMLCanvasElement | undefined;
	#composing = false;
	#demand: ProjectionDemand = 'active';
	#demandSent: ProjectionDemand | undefined;
	#devicePixelRatio = 1;
	#frameHandle: ScheduledFrame | undefined;
	#input: HTMLTextAreaElement | undefined;
	#lastBeforeInput: SentInput | undefined;
	#lastComposition: SentInput | undefined;
	#lastGeometry: ResizeGeometry | undefined;
	#lastKeyInput: SentInput | undefined;
	#lastPaste: (SentInput & { readonly source: 'beforeinput' | 'clipboard' }) | undefined;
	#listenerDisposers: Array<() => void> = [];
	#pendingExit: TerminalExited | undefined;
	#phase: ProjectedTerminalPhase = ProjectedTerminalPhase.constructed;
	#renderer: WebGPURenderer | undefined;
	#resizeObserver: ResizeObserver | undefined;
	#root: HTMLElement | undefined;

	constructor(options: ProjectedTerminalOptions) {
		this.#options = options;
		this.#hostEpoch = options.hostEpoch;
		this.#attachmentId = options.attachmentId;
		this.#creationId = options.creationId;
		this.#store = new PackedTerminalStore(options.hostEpoch, options.attachmentId);
		this.#terminalId = options.terminalId;
		if (options.autoActivate === false) this.#demand = 'paused';
	}

	get phase(): ProjectedTerminalPhase {
		return this.#phase;
	}

	get metadata(): ProjectedTerminalMetadata | null {
		return this.#store.metadata;
	}

	get canvas(): HTMLCanvasElement | undefined {
		return this.#canvas;
	}

	get textarea(): HTMLTextAreaElement | undefined {
		return this.#input;
	}

	readonly onError: IEvent<ProjectedTerminalErrorEvent> = this.#errorEmitter.event;
	readonly onExit: IEvent<TerminalExited> = this.#exitEmitter.event;
	readonly onFrameTelemetry: IEvent<ProjectedTerminalFrameTelemetry> =
		this.#frameTelemetryEmitter.event;
	readonly onMetadataChange: IEvent<ProjectedTerminalMetadata> = this.#metadataEmitter.event;
	readonly onRendererHealth: IEvent<RendererHealthEvent> = this.#healthEmitter.event;

	async open(root: HTMLElement): Promise<void> {
		if (this.#phase !== ProjectedTerminalPhase.constructed) {
			throw new Error(`ProjectedTerminal cannot open from phase ${this.#phase}`);
		}
		this.#phase = ProjectedTerminalPhase.opening;
		this.#root = root;
		try {
			const document = root.ownerDocument;
			const view = document.defaultView;
			this.#devicePixelRatio =
				this.#options.renderer?.devicePixelRatio ?? view?.devicePixelRatio ?? 1;
			const canvas = document.createElement('canvas');
			canvas.setAttribute('aria-hidden', 'true');
			canvas.style.display = 'block';
			canvas.style.outline = 'none';
			root.appendChild(canvas);
			this.#canvas = canvas;

			const input = document.createElement('textarea');
			input.setAttribute('aria-label', this.#options.labels.input);
			input.setAttribute('autocapitalize', 'off');
			input.setAttribute('autocomplete', 'off');
			input.setAttribute('autocorrect', 'off');
			input.setAttribute('spellcheck', 'false');
			input.style.position = 'absolute';
			input.style.width = '1px';
			input.style.height = '1px';
			input.style.padding = '0';
			input.style.border = '0';
			input.style.margin = '-1px';
			input.style.opacity = '0';
			input.style.clipPath = 'inset(50%)';
			input.style.overflow = 'hidden';
			input.style.whiteSpace = 'nowrap';
			input.style.resize = 'none';
			root.appendChild(input);
			this.#input = input;

			const accessibilityRoot = document.createElement('div');
			accessibilityRoot.setAttribute('aria-label', this.#options.labels.screen);
			accessibilityRoot.setAttribute('aria-live', 'polite');
			accessibilityRoot.setAttribute('aria-relevant', 'additions text');
			accessibilityRoot.setAttribute('role', 'log');
			accessibilityRoot.style.position = 'absolute';
			accessibilityRoot.style.width = '1px';
			accessibilityRoot.style.height = '1px';
			accessibilityRoot.style.margin = '-1px';
			accessibilityRoot.style.padding = '0';
			accessibilityRoot.style.border = '0';
			accessibilityRoot.style.overflow = 'hidden';
			accessibilityRoot.style.clipPath = 'inset(50%)';
			accessibilityRoot.style.whiteSpace = 'pre';
			root.appendChild(accessibilityRoot);
			this.#accessibilityRoot = accessibilityRoot;

			const renderer = new WebGPURenderer(canvas, this.#options.renderer);
			renderer.setHealthChangeCallback((event) => this.#healthEmitter.fire(event));
			renderer.setFrameTelemetryCallback((event) => {
				const revision = this.#store.metadata?.revision;
				if (revision === undefined) return;
				this.#frameTelemetryEmitter.fire({
					...event,
					revision: CanonicalUint64TextSchema.parse(revision.toString()),
				});
			});
			renderer.setRenderRequestCallback(() => this.#scheduleRender());
			this.#renderer = renderer;
			this.#installInputListeners(canvas, input);
			await renderer.initGPU();
			if (this.#isDisposed()) return;
			this.#installResizeObserver(root);
			this.#phase = ProjectedTerminalPhase.open;
			this.#sendDemandIfNeeded();
			this.#sendResizeIfChanged();
		} catch (error) {
			this.#errorEmitter.fire({ error, operation: 'open' });
			this.dispose();
			throw error;
		}
	}

	applyFrame(frame: ArrayBuffer | Uint8Array): ProjectedFrameResult {
		if (this.#phase === ProjectedTerminalPhase.disposed) return 'disposed';
		const bytes = frame instanceof Uint8Array ? frame : new Uint8Array(frame);
		let result: ReturnType<PackedTerminalStore['apply']>;
		try {
			result = this.#store.apply(bytes);
		} catch (error) {
			this.#errorEmitter.fire({ error, operation: 'frame' });
			this.#send({
				kind: 'need-snapshot',
				payload: {
					attachmentId: this.#attachmentId,
					reason: 'decoder-rejected',
					sequence: CanonicalUint64TextSchema.parse('1'),
				},
			});
			return 'snapshot-required';
		}
		if (result.kind === 'disposed') return 'disposed';
		if (result.kind === 'snapshot-required') {
			this.#send({ kind: 'need-snapshot', payload: result.request });
			return 'snapshot-required';
		}
		if (result.kind === 'committed') {
			const metadata = this.#store.metadata;
			if (metadata) {
				this.#metadataEmitter.fire(metadata);
				this.#updateAccessibility(metadata);
			}
			if (!this.#scheduleRender()) {
				this.#errorEmitter.fire({
					error: new Error('Projected frame arrived before the rendering surface was open'),
					operation: 'frame',
				});
				return 'committed';
			}
		}
		this.#send({ kind: 'frame-applied', payload: result.acknowledgement });
		this.#finalizeExitIfCaughtUp();
		return result.kind;
	}

	setProjectionDemand(demand: ProjectionDemand): void {
		if (this.#phase === ProjectedTerminalPhase.disposed || demand === this.#demand) return;
		this.#demand = demand;
		this.#sendDemandIfNeeded();
		this.#renderer?.setSuspended(demand === 'paused');
	}

	focus(): void {
		this.#input?.focus({ preventScroll: true });
	}

	async paste(text: string, source: ProjectedPasteSource = 'api'): Promise<ProjectedPasteResult> {
		if (!text) return { kind: 'empty' };
		const { sanitized, replaced } = sanitizePaste(text);
		if (!sanitized || textEncoder.encode(sanitized).byteLength > PROTOCOL_LIMITS.maxInputBytes) {
			return { kind: 'empty' };
		}
		const review: ProjectedPasteReview = {
			bracketed: this.#store.metadata?.interaction.bracketedPaste ?? false,
			embeddedBracketTerminator: sanitized.includes(BRACKETED_PASTE_END),
			multiline: sanitized.includes('\n'),
			replacedControlCharacters: replaced,
			source,
			text: sanitized,
		};
		const protectedByBracketedPaste =
			review.bracketed && review.multiline && !review.embeddedBracketTerminator;
		const requiresReview =
			review.replacedControlCharacters > 0 ||
			review.embeddedBracketTerminator ||
			(review.multiline && !protectedByBracketedPaste);
		if (requiresReview) {
			const approved = await this.#options.reviewPaste?.(review);
			if (approved !== true) return { kind: 'blocked', review };
		}
		this.#send({
			kind: 'paste-input',
			payload: { attachmentId: this.#attachmentId, text: sanitized },
		});
		return { kind: 'sent', review };
	}

	notifyExited(event: TerminalExited): void {
		if (
			event.identity.creationId !== this.#creationId ||
			event.identity.hostEpoch !== this.#hostEpoch ||
			event.identity.terminalId !== this.#terminalId
		) {
			this.#errorEmitter.fire({
				error: new Error('Terminal exit identity does not match the projected terminal'),
				operation: 'frame',
			});
			return;
		}
		this.#pendingExit = event;
		this.#finalizeExitIfCaughtUp();
	}

	dispose(): void {
		if (this.#phase === ProjectedTerminalPhase.disposed) return;
		if (this.#demandSent === 'active') {
			this.#demand = 'paused';
			this.#sendDemandIfNeeded();
		}
		this.#phase = ProjectedTerminalPhase.disposed;
		if (this.#frameHandle?.kind === 'animation-frame') {
			this.#root?.ownerDocument.defaultView?.cancelAnimationFrame(this.#frameHandle.value);
		} else if (this.#frameHandle?.kind === 'timeout') {
			globalThis.clearTimeout(this.#frameHandle.value);
		}
		this.#frameHandle = undefined;
		this.#resizeObserver?.disconnect();
		this.#resizeObserver = undefined;
		for (const dispose of this.#listenerDisposers.splice(0)) dispose();
		this.#renderer?.dispose();
		this.#renderer = undefined;
		this.#store.dispose();
		this.#canvas?.remove();
		this.#input?.remove();
		this.#accessibilityRoot?.remove();
		this.#accessibilityRows.length = 0;
		this.#canvas = undefined;
		this.#input = undefined;
		this.#accessibilityRoot = undefined;
		this.#root = undefined;
		this.#errorEmitter.dispose();
		this.#exitEmitter.dispose();
		this.#frameTelemetryEmitter.dispose();
		this.#healthEmitter.dispose();
		this.#metadataEmitter.dispose();
	}

	#scheduleRender(): boolean {
		if (this.#phase === ProjectedTerminalPhase.disposed || !this.#renderer) return false;
		if (this.#frameHandle !== undefined) return true;
		const callback = (): void => {
			this.#frameHandle = undefined;
			if (this.#phase === ProjectedTerminalPhase.disposed) return;
			try {
				this.#renderer?.renderPacked(this.#store);
			} catch (error) {
				this.#errorEmitter.fire({ error, operation: 'render' });
			}
		};
		const view = this.#root?.ownerDocument.defaultView;
		this.#frameHandle =
			typeof view?.requestAnimationFrame === 'function'
				? { kind: 'animation-frame', value: view.requestAnimationFrame(callback) }
				: { kind: 'timeout', value: globalThis.setTimeout(callback, 16) };
		return true;
	}

	#isDisposed(): boolean {
		return this.#phase === ProjectedTerminalPhase.disposed;
	}

	#send(command: ProjectedTerminalCommand): void {
		try {
			const pending = this.#options.port.send(command);
			void pending?.catch((error) => this.#errorEmitter.fire({ error, operation: 'command' }));
		} catch (error) {
			this.#errorEmitter.fire({ error, operation: 'command' });
		}
	}

	#sendDemandIfNeeded(): void {
		if (this.#phase !== ProjectedTerminalPhase.open || this.#demand === this.#demandSent) return;
		this.#send({
			kind: 'set-projection-demand',
			payload: { attachmentId: this.#attachmentId, demand: this.#demand },
		});
		this.#demandSent = this.#demand;
	}

	#installInputListeners(canvas: HTMLCanvasElement, input: HTMLTextAreaElement): void {
		if (this.#options.inputEnabled === false) return;
		this.#listen(input, 'keydown', this.#handleKeyDown);
		this.#listen(input, 'keyup', this.#handleKeyUp);
		this.#listen(input, 'beforeinput', this.#handleBeforeInput);
		this.#listen(input, 'compositionstart', this.#handleCompositionStart);
		this.#listen(input, 'compositionupdate', this.#handleCompositionUpdate);
		this.#listen(input, 'compositionend', this.#handleCompositionEnd);
		this.#listen(input, 'paste', this.#handlePaste);
		this.#listen(input, 'focus', this.#handleFocus);
		this.#listen(input, 'blur', this.#handleBlur);
		this.#listen(canvas, 'pointerdown', this.#handlePointerDown);
		this.#listen(canvas, 'pointerup', this.#handlePointerUp);
		this.#listen(canvas, 'pointermove', this.#handlePointerMove);
		this.#listen(canvas, 'wheel', this.#handleWheel, { passive: false });
	}

	#listen<K extends keyof HTMLElementEventMap>(
		target: HTMLElement,
		type: K,
		listener: (this: HTMLElement, event: HTMLElementEventMap[K]) => unknown,
		options?: AddEventListenerOptions,
	): void {
		target.addEventListener(type, listener, options);
		this.#listenerDisposers.push(() => target.removeEventListener(type, listener, options));
	}

	readonly #handleKeyDown = (event: KeyboardEvent): void => {
		this.#trackModifier(event.code, true);
		if (this.#composing || event.isComposing || event.keyCode === 229) return;
		if ((event.ctrlKey || event.metaKey) && event.code === 'KeyV') return;
		const facts = this.#keyFacts(event);
		if (!facts) return;
		this.#pressedKeys.set(event.code, facts);
		this.#sendKey(facts, event.repeat ? KeyAction.repeat : KeyAction.press);
		if (facts.text) this.#lastKeyInput = { text: facts.text, time: projectedNow() };
		event.preventDefault();
		event.stopPropagation();
	};

	readonly #handleKeyUp = (event: KeyboardEvent): void => {
		const facts = this.#pressedKeys.get(event.code) ?? this.#keyFacts(event);
		if (facts) {
			this.#sendKey(facts, KeyAction.release);
			event.preventDefault();
			event.stopPropagation();
		}
		this.#pressedKeys.delete(event.code);
		this.#trackModifier(event.code, false);
	};

	#sendKey(facts: KeyFacts, action: KeyInput['action']): void {
		this.#send({
			kind: 'key-input',
			payload: { action, attachmentId: this.#attachmentId, ...facts },
		});
	}

	#keyFacts(event: KeyboardEvent): KeyFacts | null {
		let code: TerminalKeyCodeValue | null = terminalKeyCodes.get(event.code) ?? null;
		if (code === null && event.key.length === 1) code = 'Unidentified';
		if (!code) return null;
		const text = event.key.length === 1 ? event.key : '';
		const unshiftedCodepoint = unshiftedCharacter(event.code)?.codePointAt(0) ?? 0;
		const modifiers = this.#modifiers(event);
		const consumedModifiers = emptyModifiers();
		if (event.shiftKey && text && unshiftedCodepoint !== text.codePointAt(0)) {
			consumedModifiers.shift = true;
			consumedModifiers.shiftRight = modifiers.shiftRight;
		}
		if (event.getModifierState?.('AltGraph')) {
			consumedModifiers.alt = true;
			consumedModifiers.control = true;
		}
		return {
			composing: false,
			consumedModifiers,
			key: code,
			modifiers,
			text,
			unshiftedCodepoint,
		};
	}

	#modifiers(event: KeyboardEvent | MouseEvent): TerminalModifiers {
		return {
			alt: event.altKey,
			altRight: this.#pressedModifierCodes.has('AltRight'),
			capsLock: event.getModifierState?.('CapsLock') ?? false,
			control: event.ctrlKey,
			controlRight: this.#pressedModifierCodes.has('ControlRight'),
			meta: event.metaKey,
			metaRight: this.#pressedModifierCodes.has('MetaRight'),
			numLock: event.getModifierState?.('NumLock') ?? false,
			shift: event.shiftKey,
			shiftRight: this.#pressedModifierCodes.has('ShiftRight'),
		};
	}

	#trackModifier(code: string, pressed: boolean): void {
		if (!['AltRight', 'ControlRight', 'MetaRight', 'ShiftRight'].includes(code)) return;
		if (pressed) this.#pressedModifierCodes.add(code);
		else this.#pressedModifierCodes.delete(code);
	}

	readonly #handleBeforeInput = (event: InputEvent): void => {
		if (this.#composing || event.isComposing) return;
		const data = event.data ?? '';
		if (event.inputType === 'insertFromPaste' && data) {
			event.preventDefault();
			event.stopPropagation();
			if (!this.#isDuplicatePaste(data, 'beforeinput')) {
				void this.paste(data, 'beforeinput');
				this.#lastPaste = { source: 'beforeinput', text: data, time: projectedNow() };
			}
			return;
		}
		if (event.inputType === 'insertLineBreak' || event.inputType === 'insertParagraph') {
			this.#sendSyntheticKey('Enter');
			event.preventDefault();
			event.stopPropagation();
			return;
		}
		if (event.inputType === 'deleteContentBackward') {
			this.#sendSyntheticKey('Backspace');
			event.preventDefault();
			event.stopPropagation();
			return;
		}
		if (event.inputType === 'deleteContentForward') {
			this.#sendSyntheticKey('Delete');
			event.preventDefault();
			event.stopPropagation();
			return;
		}
		if (event.inputType !== 'insertText' && event.inputType !== 'insertReplacementText') return;
		const text = data;
		if (
			!text ||
			this.#isRecent(this.#lastKeyInput, text) ||
			this.#isRecent(this.#lastComposition, text)
		) {
			event.preventDefault();
			return;
		}
		this.#sendText(text);
		this.#lastBeforeInput = { text, time: projectedNow() };
		event.preventDefault();
		event.stopPropagation();
		this.#clearInput();
	};

	readonly #handleCompositionStart = (event: CompositionEvent): void => {
		this.#composing = true;
		this.#updatePreedit(event.data ?? '');
	};

	readonly #handleCompositionUpdate = (event: CompositionEvent): void => {
		this.#updatePreedit(event.data ?? '');
	};

	readonly #handleCompositionEnd = (event: CompositionEvent): void => {
		const wasComposing = this.#composing;
		this.#composing = false;
		this.#renderer?.setPreedit([]);
		const data = event.data ?? '';
		if (
			data &&
			!shouldSuppressComposingControlInput(data, wasComposing) &&
			!this.#isRecent(this.#lastBeforeInput, data)
		) {
			this.#sendText(data);
			this.#lastComposition = { text: data, time: projectedNow() };
		}
		this.#clearInput();
	};

	#updatePreedit(data: string): void {
		const input = this.#input;
		const selection = {
			start: input?.selectionStart ?? data.length,
			end: input?.selectionEnd ?? data.length,
		};
		const state = buildPreeditState(data, selection, unicodeCellWidth);
		this.#renderer?.setPreedit(state.cells, state.selection);
	}

	#sendText(text: string): void {
		if (textEncoder.encode(text).byteLength > PROTOCOL_LIMITS.maxInputBytes) return;
		this.#send({
			kind: 'text-input',
			payload: { attachmentId: this.#attachmentId, text },
		});
	}

	#sendSyntheticKey(key: TerminalKeyCodeValue): void {
		this.#sendKey(
			{
				composing: false,
				consumedModifiers: emptyModifiers(),
				key,
				modifiers: emptyModifiers(),
				text: '',
				unshiftedCodepoint: 0,
			},
			KeyAction.press,
		);
	}

	readonly #handlePaste = (event: ClipboardEvent): void => {
		event.preventDefault();
		event.stopPropagation();
		const text = event.clipboardData?.getData('text/plain') ?? '';
		if (!text || this.#isDuplicatePaste(text, 'clipboard')) return;
		void this.paste(text, 'clipboard');
		this.#lastPaste = { source: 'clipboard', text, time: projectedNow() };
	};

	#isDuplicatePaste(text: string, source: 'beforeinput' | 'clipboard'): boolean {
		const previous = this.#lastPaste;
		return previous !== undefined && previous.source !== source && this.#isRecent(previous, text);
	}

	#isRecent(previous: SentInput | undefined, text: string): boolean {
		return (
			previous !== undefined &&
			previous.text === text &&
			projectedNow() - previous.time < INPUT_DEDUPLICATION_MS
		);
	}

	readonly #handleFocus = (): void => {
		this.#renderer?.setFocused(true);
		this.#send({
			kind: 'focus-input',
			payload: { attachmentId: this.#attachmentId, focused: true },
		});
	};

	readonly #handleBlur = (): void => {
		this.#renderer?.setFocused(false);
		this.#send({
			kind: 'focus-input',
			payload: { attachmentId: this.#attachmentId, focused: false },
		});
		this.#pressedKeys.clear();
		this.#pressedModifierCodes.clear();
	};

	readonly #handlePointerDown = (event: PointerEvent): void => {
		this.focus();
		if (!this.#mouseOwned('press')) return;
		this.#sendMouse(event, MouseAction.press, domMouseButton(event.button));
		event.preventDefault();
	};

	readonly #handlePointerUp = (event: PointerEvent): void => {
		if (!this.#mouseOwned('release')) return;
		this.#sendMouse(event, MouseAction.release, domMouseButton(event.button));
		event.preventDefault();
	};

	readonly #handlePointerMove = (event: PointerEvent): void => {
		if (!this.#mouseOwned('motion', event.buttons)) return;
		this.#sendMouse(event, MouseAction.motion, null);
		event.preventDefault();
	};

	readonly #handleWheel = (event: WheelEvent): void => {
		if (!this.#mouseOwned('press') || event.deltaY === 0) return;
		this.#sendMouse(
			event,
			MouseAction.press,
			event.deltaY < 0 ? MouseButton.four : MouseButton.five,
		);
		event.preventDefault();
	};

	#mouseOwned(action: MouseInput['action'], buttons = 0): boolean {
		const interaction = this.#store.metadata?.interaction;
		if (!interaction) return false;
		if (action === MouseAction.release) return true;
		if (action === MouseAction.motion) {
			return interaction.mouseAny || (buttons !== 0 && interaction.mouseButton);
		}
		return (
			interaction.mouseAny ||
			interaction.mouseButton ||
			interaction.mouseNormal ||
			interaction.mouseX10
		);
	}

	#sendMouse(event: MouseEvent, action: MouseInput['action'], button: MouseInput['button']): void {
		const metadata = this.#store.metadata;
		const canvas = this.#canvas;
		const renderer = this.#renderer;
		if (!metadata || !canvas || !renderer) return;
		const rectangle = canvas.getBoundingClientRect();
		const x = clamp(
			((event.clientX - rectangle.left) / renderer.charWidth) * metadata.cellPixelWidth,
			0,
			metadata.columns * metadata.cellPixelWidth,
		);
		const y = clamp(
			((event.clientY - rectangle.top) / renderer.charHeight) * metadata.cellPixelHeight,
			0,
			metadata.rows * metadata.cellPixelHeight,
		);
		this.#send({
			kind: 'mouse-input',
			payload: {
				action,
				attachmentId: this.#attachmentId,
				button,
				modifiers: this.#modifiers(event),
				pressedButtons: domPressedButtons(event.buttons),
				x,
				y,
			},
		});
	}

	#installResizeObserver(root: HTMLElement): void {
		if (this.#options.canResize !== true || typeof ResizeObserver === 'undefined') return;
		this.#resizeObserver = new ResizeObserver(() => this.#sendResizeIfChanged());
		this.#resizeObserver.observe(root);
	}

	#sendResizeIfChanged(): void {
		if (this.#phase !== ProjectedTerminalPhase.open || this.#options.canResize !== true) return;
		const root = this.#root;
		const renderer = this.#renderer;
		if (!root || !renderer || root.clientWidth <= 0 || root.clientHeight <= 0) return;
		const metrics = renderer.getMetrics();
		const cellPixelHeight = clamp(
			Math.round(metrics.height * this.#devicePixelRatio),
			1,
			PROTOCOL_LIMITS.maxCellPixelDimension,
		);
		const cellPixelWidth = clamp(
			Math.round(metrics.width * this.#devicePixelRatio),
			1,
			PROTOCOL_LIMITS.maxCellPixelDimension,
		);
		const columns = clamp(
			Math.floor(root.clientWidth / metrics.width),
			1,
			Math.min(
				PROTOCOL_LIMITS.maxFrameColumns,
				Math.floor(MAX_PTY_PIXEL_DIMENSION / cellPixelWidth),
			),
		);
		const rows = clamp(
			Math.floor(root.clientHeight / metrics.height),
			1,
			Math.min(
				PROTOCOL_LIMITS.maxFrameRows,
				Math.floor(PROTOCOL_LIMITS.maxFrameCells / columns),
				Math.floor(MAX_PTY_PIXEL_DIMENSION / cellPixelHeight),
			),
		);
		const geometry: ResizeGeometry = {
			cellPixelHeight,
			cellPixelWidth,
			columns,
			rows,
		};
		if (sameGeometry(geometry, this.#lastGeometry)) return;
		this.#lastGeometry = geometry;
		this.#send({
			kind: 'resize-terminal',
			payload: { attachmentId: this.#attachmentId, ...geometry },
		});
	}

	#updateAccessibility(metadata: ProjectedTerminalMetadata): void {
		const root = this.#accessibilityRoot;
		if (!root) return;
		while (this.#accessibilityRows.length < metadata.rows) {
			const row = root.ownerDocument.createElement('div');
			row.setAttribute('role', 'listitem');
			root.appendChild(row);
			this.#accessibilityRows.push(row);
		}
		while (this.#accessibilityRows.length > metadata.rows) {
			this.#accessibilityRows.pop()?.remove();
		}
		for (let rowIndex = 0; rowIndex < metadata.rows; rowIndex++) {
			const row = this.#store.rows[rowIndex];
			const element = this.#accessibilityRows[rowIndex];
			if (!row || !element || (!row.dirty && !this.#store.needsFullRedraw)) continue;
			let text = '';
			for (let column = 0; column < metadata.columns; column++) {
				if (projectedCellDisplayWidth(row.widths[column] ?? 0) > 0) {
					text += row.grapheme(column) || ' ';
				}
			}
			element.textContent = text.trimEnd() || ' ';
		}
		root.setAttribute('aria-rowcount', String(metadata.rows));
	}

	#finalizeExitIfCaughtUp(): void {
		const event = this.#pendingExit;
		const revision = this.#store.metadata?.revision;
		if (!event || revision === undefined || revision < BigInt(event.finalRevision)) return;
		this.#pendingExit = undefined;
		this.#exitEmitter.fire(event);
	}

	#clearInput(): void {
		if (this.#input) this.#input.value = '';
	}
}

function emptyModifiers(): TerminalModifiers & {
	alt: boolean;
	altRight: boolean;
	capsLock: boolean;
	control: boolean;
	controlRight: boolean;
	meta: boolean;
	metaRight: boolean;
	numLock: boolean;
	shift: boolean;
	shiftRight: boolean;
} {
	return {
		alt: false,
		altRight: false,
		capsLock: false,
		control: false,
		controlRight: false,
		meta: false,
		metaRight: false,
		numLock: false,
		shift: false,
		shiftRight: false,
	};
}

function unshiftedCharacter(code: string): string | undefined {
	if (/^Key[A-Z]$/u.test(code)) return code.slice(3).toLowerCase();
	return UNSHIFTED_CHARACTERS[code];
}

const UNSHIFTED_CHARACTERS: Readonly<Record<string, string>> = {
	Backquote: '`',
	Backslash: '\\',
	BracketLeft: '[',
	BracketRight: ']',
	Comma: ',',
	Digit0: '0',
	Digit1: '1',
	Digit2: '2',
	Digit3: '3',
	Digit4: '4',
	Digit5: '5',
	Digit6: '6',
	Digit7: '7',
	Digit8: '8',
	Digit9: '9',
	Equal: '=',
	Minus: '-',
	Period: '.',
	Quote: "'",
	Semicolon: ';',
	Slash: '/',
	Space: ' ',
};

function domMouseButton(button: number): MouseInput['button'] {
	return DOM_MOUSE_BUTTONS[button] ?? null;
}

const DOM_MOUSE_BUTTONS: readonly MouseInput['button'][] = [
	MouseButton.left,
	MouseButton.middle,
	MouseButton.right,
	MouseButton.four,
	MouseButton.five,
	MouseButton.six,
	MouseButton.seven,
	MouseButton.eight,
	MouseButton.nine,
	MouseButton.ten,
	MouseButton.eleven,
];

function domPressedButtons(buttons: number): number {
	let normalized = 0;
	if ((buttons & 1) !== 0) normalized |= 1 << (MouseButton.left - 1);
	if ((buttons & 2) !== 0) normalized |= 1 << (MouseButton.right - 1);
	if ((buttons & 4) !== 0) normalized |= 1 << (MouseButton.middle - 1);
	if ((buttons & 8) !== 0) normalized |= 1 << (MouseButton.four - 1);
	if ((buttons & 16) !== 0) normalized |= 1 << (MouseButton.five - 1);
	return normalized;
}

function sanitizePaste(text: string): { readonly replaced: number; readonly sanitized: string } {
	let replaced = 0;
	let sanitized = '';
	for (const character of text) {
		const codepoint = character.codePointAt(0) ?? 0;
		const unsafe =
			codepoint <= 0x08 ||
			codepoint === 0x0b ||
			codepoint === 0x0c ||
			(codepoint >= 0x0e && codepoint <= 0x1f) ||
			(codepoint >= 0x7f && codepoint <= 0x9f);
		if (unsafe) replaced++;
		sanitized += unsafe ? ' ' : character;
	}
	return { replaced, sanitized };
}

function unicodeCellWidth(codepoint: number): number {
	if (codepoint === 0 || codepoint === 0x200d) return 0;
	const character = String.fromCodePoint(codepoint);
	if (/\p{Mark}/u.test(character)) return 0;
	return codepoint >= 0x1100 &&
		(codepoint <= 0x115f ||
			codepoint === 0x2329 ||
			codepoint === 0x232a ||
			(codepoint >= 0x2e80 && codepoint <= 0xa4cf) ||
			(codepoint >= 0xac00 && codepoint <= 0xd7a3) ||
			(codepoint >= 0xf900 && codepoint <= 0xfaff) ||
			(codepoint >= 0xfe10 && codepoint <= 0xfe19) ||
			(codepoint >= 0xfe30 && codepoint <= 0xfe6f) ||
			(codepoint >= 0xff00 && codepoint <= 0xff60) ||
			(codepoint >= 0xffe0 && codepoint <= 0xffe6) ||
			(codepoint >= 0x1f300 && codepoint <= 0x1faff))
		? 2
		: 1;
}

function sameGeometry(left: ResizeGeometry, right: ResizeGeometry | undefined): boolean {
	return (
		right !== undefined &&
		left.cellPixelHeight === right.cellPixelHeight &&
		left.cellPixelWidth === right.cellPixelWidth &&
		left.columns === right.columns &&
		left.rows === right.rows
	);
}

function clamp(value: number, minimum: number, maximum: number): number {
	return Math.max(minimum, Math.min(maximum, value));
}

function projectedNow(): number {
	return typeof performance !== 'undefined' && typeof performance.now === 'function'
		? performance.now()
		: Date.now();
}
