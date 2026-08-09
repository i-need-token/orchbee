import {
	KeyAction,
	CanonicalUint64TextSchema,
	decodeTerminalRowWindow,
	MouseAction,
	MouseButton,
	PROTOCOL_LIMITS,
	TerminalFrameKind,
	TerminalKeyCode,
	viewTerminalRowWindow,
	type AttachmentId,
	type CanonicalUint64Text,
	type CreationId,
	type FocusInput,
	type FrameApplied,
	type HistoryChanged,
	type HistoryObserved,
	type HostEpoch,
	type KeyInput,
	type MouseInput,
	type NeedSnapshot,
	type PasteInput,
	type ProjectionDemand,
	type ReadRows,
	type ResizeTerminal,
	type RowsApplied,
	type SelectionChanged,
	type SelectionCommand,
	type SelectionGesture,
	type SetHistoryDemand,
	type SetProjectionDemand,
	type TerminalExited,
	type TerminalId,
	type TerminalKeyCodeValue,
	type TerminalModifiers,
	type TextInput,
} from '@orchbee/terminal-protocol';

import { EventEmitter, type IEvent } from '../EventEmitter';
import {
	bufferRowsFromDecodedWindow,
	validateBufferRowsRequest,
	type BufferRowsResult,
	type TerminalBufferRowsRequest,
	type TerminalBufferRowsSnapshot,
} from '../buffer-rows';
import {
	buildPreeditState,
	calculateInputAnchor,
	OrderedInputDispatcher,
	shouldSuppressComposingControlInput,
} from '../input';
import {
	type RendererFrameTelemetry,
	type RendererHealthEvent,
	WebGPURenderer,
} from '../rendering';
import {
	createProjectedSearchWorker,
	ProjectedSearchController,
	type ProjectedSearchWorkerFactory,
	type ProjectedTerminalSearchEvent,
	type TerminalSearchMatch,
	type TerminalSearchOptions,
} from '../search';
import {
	detectTerminalRowLinks,
	isPermittedExternalTerminalTarget,
	terminalLinkContains,
} from '../terminal-row-links';
import {
	DEFAULT_RENDERER_FONT_FAMILY,
	DEFAULT_RENDERER_FONT_SIZE,
	DEFAULT_RENDERER_FONT_WEIGHT,
} from '../types';
import type { ILink, RendererOptions } from '../types';
import {
	PackedTerminalStore,
	PackedTerminalViewport,
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
const MAX_HISTORY_CACHE_BYTES = 8 * 1024 * 1024;
const HISTORY_OVERSCAN_ROWS = 16;

export type ProjectedTerminalCommand =
	| { readonly kind: 'focus-input'; readonly payload: FocusInput }
	| { readonly kind: 'frame-applied'; readonly payload: FrameApplied }
	| { readonly kind: 'key-input'; readonly payload: KeyInput }
	| { readonly kind: 'mouse-input'; readonly payload: MouseInput }
	| { readonly kind: 'need-snapshot'; readonly payload: NeedSnapshot }
	| { readonly kind: 'paste-input'; readonly payload: PasteInput }
	| { readonly kind: 'read-rows'; readonly payload: ReadRows }
	| { readonly kind: 'resize-terminal'; readonly payload: ResizeTerminal }
	| { readonly kind: 'rows-applied'; readonly payload: RowsApplied }
	| { readonly kind: 'history-observed'; readonly payload: HistoryObserved }
	| { readonly kind: 'selection-command'; readonly payload: SelectionCommand }
	| { readonly kind: 'selection-gesture'; readonly payload: SelectionGesture }
	| {
			readonly kind: 'selection-copy';
			readonly payload: {
				readonly attachmentId: AttachmentId;
				readonly requestSequence: CanonicalUint64Text;
			};
	  }
	| { readonly kind: 'set-history-demand'; readonly payload: SetHistoryDemand }
	| { readonly kind: 'set-projection-demand'; readonly payload: SetProjectionDemand }
	| {
			readonly kind: 'link-open';
			readonly payload: {
				readonly attachmentId: AttachmentId;
				readonly requestSequence: CanonicalUint64Text;
				readonly target: string;
			};
	  }
	| { readonly kind: 'text-input'; readonly payload: TextInput };

export interface ProjectedTerminalCommandPort {
	send(command: ProjectedTerminalCommand): Promise<void> | void;
}

export interface ProjectedTerminalLabels {
	/** Localized current/retained row-range description for assistive technology. */
	readonly historyRange?: (start: bigint, end: bigint, total: bigint) => string;
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
	/** Trust boundary for OBPB 0.0.1 render buffers. Omitted means strict external validation. */
	readonly projectionBufferTrust?: 'external' | 'local-host';
	readonly renderer?: RendererOptions;
	readonly reviewPaste?: (review: ProjectedPasteReview) => boolean | Promise<boolean>;
	readonly searchWorkerFactory?: ProjectedSearchWorkerFactory;
	readonly terminalId: TerminalId;
}

export interface ProjectedTerminalErrorEvent {
	readonly error: unknown;
	readonly operation:
		| 'command'
		| 'font'
		| 'frame'
		| 'history'
		| 'open'
		| 'render'
		| 'resize'
		| 'selection';
}

export interface ProjectedBrokerFailure {
	readonly code: string;
	readonly message: string;
	readonly operation?:
		| 'connection'
		| 'copy'
		| 'history'
		| 'input'
		| 'link'
		| 'protocol'
		| 'resize'
		| 'selection'
		| undefined;
	readonly requestSequence?: CanonicalUint64Text | undefined;
	readonly retryable: boolean;
}

export interface ProjectedTerminalLinkOpenRequest {
	readonly target: Extract<NonNullable<ILink['target']>, { readonly kind: 'url' }>;
}

export type ProjectedTerminalSideEffectResult =
	| { readonly byteLength?: number; readonly kind: 'completed' }
	| { readonly code: string; readonly kind: 'failed'; readonly retryable: boolean }
	| { readonly kind: 'cancelled' };

export interface ProjectedTerminalFrameTelemetry extends RendererFrameTelemetry {
	readonly revision: CanonicalUint64Text;
}

export type ProjectedFrameResult = 'committed' | 'disposed' | 'duplicate' | 'snapshot-required';

export type ProjectedRowsResult = 'committed' | 'ignored' | 'rejected';

export const ProjectedHistoryPhase = {
	disposed: 'disposed',
	followingBottom: 'following-bottom',
	recovering: 'recovering',
	requestingHistory: 'requesting-history',
	viewingHistory: 'viewing-history',
} as const;

export type ProjectedHistoryPhase =
	(typeof ProjectedHistoryPhase)[keyof typeof ProjectedHistoryPhase];

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

interface BufferRowFlight {
	aborted: boolean;
	readonly kind: 'buffer';
	readonly maximumBytes: number;
	readonly removeAbortListener: () => void;
	readonly request: TerminalBufferRowsRequest;
	readonly requestSequence: CanonicalUint64Text;
	readonly resolve: (result: BufferRowsResult) => void;
}

interface ViewportRowFlight {
	readonly kind: 'viewport';
	readonly maximumBytes: number;
	readonly overscanned: boolean;
	readonly requestSequence: CanonicalUint64Text;
	readonly visibleRowCount: number;
	readonly visibleStartRow: bigint;
}

type RowFlight = BufferRowFlight | ViewportRowFlight;

interface PendingSelectionGesture {
	readonly action: SelectionGesture['action'];
	readonly clickCount: number;
	readonly point: SelectionGesture['point'];
	readonly rectangle: boolean;
}

interface PendingLinkPoint {
	readonly column: number;
	readonly control: boolean;
	readonly meta: boolean;
	readonly row: bigint;
}

interface ProjectedLinkCache {
	readonly endRow: bigint;
	readonly generation: bigint;
	readonly links: readonly ILink[];
	readonly revision: bigint;
	readonly startRow: bigint;
}

type ScheduledFrame =
	| { readonly kind: 'animation-frame'; readonly value: number }
	| { readonly kind: 'timeout'; readonly value: ReturnType<typeof setTimeout> };

export class ProjectedTerminal {
	readonly #accessibilityColumnMappings: number[][] = [];
	readonly #accessibilityRows: Element[] = [];
	readonly #attachmentId: AttachmentId;
	readonly #creationId: CreationId;
	readonly #copyCompletions = new Map<
		CanonicalUint64Text,
		(result: ProjectedTerminalSideEffectResult) => void
	>();
	readonly #errorEmitter = new EventEmitter<ProjectedTerminalErrorEvent>();
	readonly #exitEmitter = new EventEmitter<TerminalExited>();
	readonly #frameTelemetryEmitter = new EventEmitter<ProjectedTerminalFrameTelemetry>();
	readonly #healthEmitter = new EventEmitter<RendererHealthEvent>();
	readonly #historyEmitter = new EventEmitter<HistoryChanged>();
	readonly #hostEpoch: HostEpoch;
	readonly #inputDispatcher = new OrderedInputDispatcher((error) =>
		this.#errorEmitter.fire({ error, operation: 'command' }),
	);
	readonly #metadataEmitter = new EventEmitter<ProjectedTerminalMetadata>();
	readonly #linkOpenEmitter = new EventEmitter<ProjectedTerminalLinkOpenRequest>();
	readonly #linkCompletions = new Map<
		CanonicalUint64Text,
		(result: ProjectedTerminalSideEffectResult) => void
	>();
	readonly #options: ProjectedTerminalOptions;
	readonly #pressedKeys = new Map<string, KeyFacts>();
	readonly #pressedModifierCodes = new Set<string>();
	readonly #store: PackedTerminalStore;
	readonly #selectionEmitter = new EventEmitter<SelectionChanged>();
	readonly #selectionCopiedEmitter = new EventEmitter<{
		readonly byteLength: number;
		readonly requestSequence: CanonicalUint64Text;
	}>();
	readonly #searchEmitter = new EventEmitter<ProjectedTerminalSearchEvent>();
	readonly #terminalId: TerminalId;
	#accessibilityRoot: HTMLDivElement | undefined;
	#canvas: HTMLCanvasElement | undefined;
	#composing = false;
	#demand: ProjectionDemand = 'active';
	#demandSent: ProjectionDemand | undefined;
	#devicePixelRatio = 1;
	#devicePixelRatioQuery: MediaQueryList | undefined;
	#frameHandle: ScheduledFrame | undefined;
	#historyGeneration = 1n;
	#historyPhase: ProjectedHistoryPhase = ProjectedHistoryPhase.followingBottom;
	#historyRevision = 0n;
	#historyStartRow = 0n;
	#historyStore: PackedTerminalViewport | undefined;
	#historyTotalRows = 0n;
	#historyRowsBefore = 0;
	#input: HTMLTextAreaElement | undefined;
	#lastBeforeInput: SentInput | undefined;
	#lastComposition: SentInput | undefined;
	#lastGeometry: ResizeGeometry | undefined;
	#lastKeyInput: SentInput | undefined;
	#lastPaste: (SentInput & { readonly source: 'beforeinput' | 'clipboard' }) | undefined;
	#listenerDisposers: Array<() => void> = [];
	#hoveredLink: ILink | undefined;
	#hoveredLinkPoint: PendingLinkPoint | undefined;
	#linkAbortController: AbortController | undefined;
	#linkCache: ProjectedLinkCache | undefined;
	#linkLookupActive = false;
	#pendingLinkPoint: PendingLinkPoint | undefined;
	#pendingExit: TerminalExited | undefined;
	#latestExitFinalRevision: bigint | undefined;
	#pendingHistoryDelta = 0;
	#pendingHistoryTargetRow: bigint | undefined;
	#pendingSelectionGesture: PendingSelectionGesture | undefined;
	#phase: ProjectedTerminalPhase = ProjectedTerminalPhase.constructed;
	#preeditCaretPixelOffset = 0;
	#preeditCellWidth = 0;
	#renderer: WebGPURenderer | undefined;
	#resizeObserver: ResizeObserver | undefined;
	#root: HTMLElement | undefined;
	#rowFlight: RowFlight | undefined;
	#rowRequestSequence = 1n;
	#selection: SelectionChanged;
	#selectionGestureHandle: ScheduledFrame | undefined;
	#selectionGestureInFlight = false;
	#selectionGestureActive = false;
	#selectionGestureLast: Omit<PendingSelectionGesture, 'action'> | undefined;
	#selectionGesturePointerId: number | undefined;
	#searchController: ProjectedSearchController | undefined;
	#sideEffectSequence = 1n;

	constructor(options: ProjectedTerminalOptions) {
		this.#options = options;
		this.#hostEpoch = options.hostEpoch;
		this.#attachmentId = options.attachmentId;
		this.#creationId = options.creationId;
		this.#store = new PackedTerminalStore(options.hostEpoch, options.attachmentId);
		this.#terminalId = options.terminalId;
		this.#selection = { attachmentId: options.attachmentId, state: 'none' };
		if (options.autoActivate === false) this.#demand = 'paused';
	}

	get phase(): ProjectedTerminalPhase {
		return this.#phase;
	}

	get metadata(): ProjectedTerminalMetadata | null {
		return this.#visibleStore().metadata;
	}

	get historyPhase(): ProjectedHistoryPhase {
		return this.#historyPhase;
	}

	get selection(): SelectionChanged {
		return this.#selection;
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
	readonly onHistoryChange: IEvent<HistoryChanged> = this.#historyEmitter.event;
	readonly onLinkOpenRequest: IEvent<ProjectedTerminalLinkOpenRequest> =
		this.#linkOpenEmitter.event;
	readonly onSelectionChange: IEvent<SelectionChanged> = this.#selectionEmitter.event;
	readonly onSelectionCopied: IEvent<{
		readonly byteLength: number;
		readonly requestSequence: CanonicalUint64Text;
	}> = this.#selectionCopiedEmitter.event;
	readonly onSearchResults: IEvent<ProjectedTerminalSearchEvent> = this.#searchEmitter.event;

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
			const fontFamily = this.#options.renderer?.fontFamily ?? DEFAULT_RENDERER_FONT_FAMILY;
			const fontSize = this.#options.renderer?.fontSize ?? DEFAULT_RENDERER_FONT_SIZE;
			const fontWeight = this.#options.renderer?.fontWeight ?? DEFAULT_RENDERER_FONT_WEIGHT;
			await this.#loadConfiguredFont(document, fontFamily, fontSize, fontWeight);
			if (this.#isDisposed()) return;
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
			input.style.color = 'transparent';
			input.style.background = 'transparent';
			input.style.caretColor = 'transparent';
			input.style.fontFamily = fontFamily;
			input.style.fontSize = `${fontSize}px`;
			input.style.fontWeight = String(fontWeight);
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
			document.addEventListener('selectionchange', this.#handleAccessibilitySelectionChange);
			this.#listenerDisposers.push(() =>
				document.removeEventListener('selectionchange', this.#handleAccessibilitySelectionChange),
			);

			const renderer = new WebGPURenderer(canvas, {
				...this.#options.renderer,
				devicePixelRatio: this.#devicePixelRatio,
			});
			renderer.setHealthChangeCallback((event) => this.#healthEmitter.fire(event));
			renderer.setFrameTelemetryCallback((event) => {
				const revision = this.#visibleStore().metadata?.revision;
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
			this.#installDevicePixelRatioListeners(view);
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
		const previousGeneration = this.#historyGeneration;
		const previousRevision = this.#historyRevision;
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
				this.#historyGeneration = metadata.generation;
				this.#historyRevision = metadata.revision;
				this.#historyTotalRows = metadata.scrollbackRows + BigInt(metadata.rows);
				if (metadata.generation !== previousGeneration || metadata.revision !== previousRevision) {
					this.#invalidateLinkCache();
					this.#searchController?.invalidate();
				}
				if (
					this.#historyPhase === ProjectedHistoryPhase.recovering &&
					this.#store.lastFrameKind === TerminalFrameKind.snapshot
				) {
					this.#finishHistoryRecovery();
				} else if (this.#historyPhase === ProjectedHistoryPhase.followingBottom) {
					this.#metadataEmitter.fire(metadata);
					this.#updateAccessibility(metadata);
				}
			}
			if (this.#historyPhase === ProjectedHistoryPhase.followingBottom && !this.#scheduleRender()) {
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

	getBufferRowsSnapshot(): TerminalBufferRowsSnapshot {
		return {
			generation: this.#historyGeneration,
			revision: this.#historyRevision,
			totalRows: this.#historyTotalRows,
		};
	}

	readBufferRows(
		request: TerminalBufferRowsRequest,
		signal: AbortSignal,
	): Promise<BufferRowsResult> {
		const snapshot = this.getBufferRowsSnapshot();
		if (signal.aborted) return Promise.resolve({ ...snapshot, kind: 'cancelled' });
		if (!validateBufferRowsRequest(request)) {
			return Promise.reject(
				new RangeError('Projected terminal buffer row request is outside the bounded contract.'),
			);
		}
		if (request.revision !== snapshot.revision || request.generation !== snapshot.generation) {
			return Promise.resolve({ ...snapshot, kind: 'stale' });
		}
		if (request.startRow >= snapshot.totalRows) {
			return Promise.resolve({ ...snapshot, kind: 'evicted' });
		}
		if (this.#phase === ProjectedTerminalPhase.disposed) {
			return Promise.resolve({ ...snapshot, kind: 'unavailable' });
		}
		if (this.#rowFlight !== undefined) {
			return Promise.resolve({ ...snapshot, kind: 'unavailable' });
		}

		return new Promise((resolve) => {
			const requestSequence = this.#nextRowRequestSequence();
			const handleAbort = (): void => {
				const flight = this.#rowFlight;
				if (flight?.kind !== 'buffer' || flight.requestSequence !== requestSequence) return;
				flight.aborted = true;
				resolve({ ...this.getBufferRowsSnapshot(), kind: 'cancelled' });
			};
			signal.addEventListener('abort', handleAbort, { once: true });
			this.#rowFlight = {
				aborted: false,
				kind: 'buffer',
				maximumBytes: request.maximumBytes,
				removeAbortListener: () => signal.removeEventListener('abort', handleAbort),
				request,
				requestSequence,
				resolve,
			};
			this.#send({
				kind: 'read-rows',
				payload: {
					attachmentId: this.#attachmentId,
					detail: request.detail,
					generation: CanonicalUint64TextSchema.parse(request.generation.toString()),
					maximumBytes: request.maximumBytes,
					origin: {
						kind: 'absolute',
						row: CanonicalUint64TextSchema.parse(request.startRow.toString()),
					},
					purpose: request.purpose ?? 'buffer',
					requestSequence,
					revision: CanonicalUint64TextSchema.parse(request.revision.toString()),
					rowCount: request.rowCount,
				},
			});
		});
	}

	applyRows(rowWindow: ArrayBuffer | Uint8Array): ProjectedRowsResult {
		const flight = this.#rowFlight;
		if (flight === undefined || this.#phase === ProjectedTerminalPhase.disposed) return 'ignored';
		const bytes = rowWindow instanceof Uint8Array ? rowWindow : new Uint8Array(rowWindow);
		this.#rowFlight = undefined;
		if (flight.kind === 'buffer') flight.removeAbortListener();
		const decoded =
			this.#options.projectionBufferTrust === 'local-host'
				? viewTerminalRowWindow(bytes)
				: decodeTerminalRowWindow(bytes);
		if (
			!decoded.ok ||
			bytes.byteLength > flight.maximumBytes ||
			(decoded.ok &&
				(decoded.value.hostEpoch !== this.#hostEpoch ||
					decoded.value.attachmentId !== this.#attachmentId ||
					decoded.value.sequence.toString() !== flight.requestSequence))
		) {
			if (flight.kind === 'buffer' && !flight.aborted) {
				flight.resolve({ ...this.getBufferRowsSnapshot(), kind: 'unavailable' });
			}
			this.#errorEmitter.fire({
				error: new Error(decoded.ok ? 'Projected row window route is invalid' : decoded.detail),
				operation: 'history',
			});
			if (flight.kind === 'viewport') this.#recoverHistory();
			return 'rejected';
		}
		const window = decoded.value;
		if (flight.kind === 'buffer') {
			if (!flight.aborted) {
				if (
					window.revision !== flight.request.revision ||
					window.generation !== flight.request.generation ||
					window.startRow !== flight.request.startRow ||
					window.detail !== flight.request.detail
				) {
					flight.resolve({ ...this.getBufferRowsSnapshot(), kind: 'stale' });
				} else {
					flight.resolve(bufferRowsFromDecodedWindow(window, bytes.byteLength));
				}
			}
			this.#dispatchPendingHistoryIntent();
			return 'committed';
		}

		if (window.detail !== 'render') {
			this.#errorEmitter.fire({
				error: new Error('Viewport history requires render-detail rows'),
				operation: 'history',
			});
			this.#recoverHistory();
			return 'rejected';
		}
		const visibleEnd = flight.visibleStartRow + BigInt(flight.visibleRowCount);
		const cacheEnd = window.startRow + BigInt(window.frame.rows);
		if (window.startRow > flight.visibleStartRow || cacheEnd < visibleEnd) {
			this.#errorEmitter.fire({
				error: new Error('Projected history window does not cover the requested viewport'),
				operation: 'history',
			});
			this.#recoverHistory();
			return 'rejected';
		}
		const staged = new PackedTerminalStore(this.#hostEpoch, this.#attachmentId);
		const result = staged.replaceProjectionBufferSnapshot(window.frame, MAX_HISTORY_CACHE_BYTES);
		if (result.kind !== 'committed') {
			staged.dispose();
			this.#errorEmitter.fire({
				error: new Error(
					result.kind === 'too-large'
						? 'Projected history window exceeds the renderer cache limit'
						: 'Projected history snapshot is invalid',
				),
				operation: 'history',
			});
			this.#recoverHistory();
			return 'rejected';
		}
		const rowOffset = Number(flight.visibleStartRow - window.startRow);
		const viewport = new PackedTerminalViewport(staged, rowOffset, flight.visibleRowCount);
		this.#historyStore?.dispose();
		this.#historyStore = viewport;
		this.#invalidateLinkCache();
		this.#historyGeneration = window.generation;
		this.#historyRevision = window.revision;
		this.#historyStartRow = flight.visibleStartRow;
		this.#historyTotalRows = window.totalRows;
		this.#historyRowsBefore = Number(
			window.totalRows - (flight.visibleStartRow + BigInt(flight.visibleRowCount)),
		);
		this.#historyPhase = ProjectedHistoryPhase.viewingHistory;
		this.#send({
			kind: 'rows-applied',
			payload: {
				attachmentId: this.#attachmentId,
				generation: CanonicalUint64TextSchema.parse(window.generation.toString()),
				requestSequence: flight.requestSequence,
				revision: CanonicalUint64TextSchema.parse(window.revision.toString()),
			},
		});
		const metadata = viewport.metadata;
		if (metadata) {
			this.#metadataEmitter.fire(metadata);
			this.#updateAccessibility(metadata);
		}
		this.#scheduleRender();
		this.#dispatchPendingHistoryIntent();
		return 'committed';
	}

	scrollLines(amount: number): void {
		if (
			!Number.isFinite(amount) ||
			amount === 0 ||
			this.#phase === ProjectedTerminalPhase.disposed
		) {
			return;
		}
		const rows = Math.trunc(amount);
		this.#invalidateLinkCache();
		if (this.#rowFlight !== undefined) {
			this.#cancelBufferFlightForViewportIntent();
			this.#pendingHistoryTargetRow = undefined;
			this.#pendingHistoryDelta = clamp(
				this.#pendingHistoryDelta + rows,
				-PROTOCOL_LIMITS.maxRowWindowRows,
				PROTOCOL_LIMITS.maxRowWindowRows,
			);
			return;
		}
		if (this.#historyPhase === ProjectedHistoryPhase.followingBottom) {
			if (rows > 0 || this.#historyTotalRows <= BigInt(this.#store.metadata?.rows ?? 0)) return;
			this.#historyRowsBefore = Math.min(Number(this.#historyTotalRows), -rows);
			this.#requestHistoryRows({ kind: 'tail', rowsBefore: this.#historyRowsBefore });
			return;
		}
		if (this.#historyPhase === ProjectedHistoryPhase.recovering) return;
		const nextRowsBefore = Math.max(0, this.#historyRowsBefore - rows);
		if (nextRowsBefore === 0) {
			this.scrollToBottom();
			return;
		}
		this.#historyRowsBefore = Math.min(Number(this.#historyTotalRows), nextRowsBefore);
		this.#requestHistoryRows({
			delta: clamp(rows, -PROTOCOL_LIMITS.maxRowWindowRows, PROTOCOL_LIMITS.maxRowWindowRows),
			kind: 'committed',
		});
	}

	scrollPages(amount: number): void {
		this.scrollLines(amount * (this.metadata?.rows ?? 1));
	}

	scrollToBottom(): void {
		if (this.#historyPhase === ProjectedHistoryPhase.followingBottom) return;
		this.#recoverHistory();
	}

	notifyHistoryChanged(event: HistoryChanged): void {
		if (event.attachmentId !== this.#attachmentId) return;
		const changed =
			BigInt(event.generation) !== this.#historyGeneration ||
			BigInt(event.revision) !== this.#historyRevision;
		this.#historyGeneration = BigInt(event.generation);
		this.#historyRevision = BigInt(event.revision);
		this.#historyTotalRows = BigInt(event.totalRows);
		this.#send({
			kind: 'history-observed',
			payload: {
				attachmentId: this.#attachmentId,
				generation: event.generation,
				revision: event.revision,
			},
		});
		this.#historyEmitter.fire(event);
		if (changed) this.#searchController?.invalidate();
		if (changed) this.#invalidateLinkCache();
		if (
			this.#historyPhase === ProjectedHistoryPhase.followingBottom ||
			this.#historyPhase === ProjectedHistoryPhase.disposed ||
			this.#rowFlight !== undefined
		) {
			return;
		}
		if (event.anchorStatus === 'valid') {
			this.#requestHistoryRows({ delta: 0, kind: 'committed' });
		} else {
			this.#requestHistoryRows({ kind: 'tail', rowsBefore: this.#historyRowsBefore });
		}
	}

	notifySelectionChanged(selection: SelectionChanged): void {
		if (selection.attachmentId !== this.#attachmentId) return;
		this.#selection = selection;
		this.#selectionGestureInFlight = false;
		this.#selectionEmitter.fire(selection);
		this.#scheduleSelectionGesture();
		this.#scheduleRender();
	}

	notifySelectionCopied(byteLength: number, requestSequence: CanonicalUint64Text): void {
		const complete = this.#copyCompletions.get(requestSequence);
		this.#copyCompletions.delete(requestSequence);
		complete?.({ byteLength, kind: 'completed' });
		this.#selectionCopiedEmitter.fire({ byteLength, requestSequence });
	}

	notifyLinkOpened(requestSequence: CanonicalUint64Text): void {
		const complete = this.#linkCompletions.get(requestSequence);
		this.#linkCompletions.delete(requestSequence);
		complete?.({ kind: 'completed' });
	}

	notifyBrokerFailure(failure: ProjectedBrokerFailure): void {
		let recoveredOverscan = false;
		const flight =
			failure.operation === undefined || failure.operation === 'history'
				? this.#rowFlight
				: undefined;
		if (flight !== undefined) {
			this.#rowFlight = undefined;
			if (flight.kind === 'buffer') {
				flight.removeAbortListener();
				if (!flight.aborted) {
					let kind: 'evicted' | 'stale' | 'too-large' | 'unavailable' = 'unavailable';
					if (failure.code === 'history-evicted') kind = 'evicted';
					else if (failure.code === 'stale-generation' || failure.code === 'stale-revision') {
						kind = 'stale';
					} else if (failure.code === 'limit-exceeded') kind = 'too-large';
					flight.resolve({ ...this.getBufferRowsSnapshot(), kind });
				}
				this.#dispatchPendingHistoryIntent();
			} else if (failure.code === 'limit-exceeded' && flight.overscanned) {
				this.#requestHistoryRows(
					{
						kind: 'absolute',
						row: CanonicalUint64TextSchema.parse(flight.visibleStartRow.toString()),
					},
					0,
				);
				recoveredOverscan = true;
			} else {
				this.#recoverHistory();
			}
		}
		if (failure.requestSequence !== undefined && failure.operation === 'copy') {
			const complete = this.#copyCompletions.get(failure.requestSequence);
			this.#copyCompletions.delete(failure.requestSequence);
			complete?.({ code: failure.code, kind: 'failed', retryable: failure.retryable });
		}
		if (failure.requestSequence !== undefined && failure.operation === 'link') {
			const complete = this.#linkCompletions.get(failure.requestSequence);
			this.#linkCompletions.delete(failure.requestSequence);
			complete?.({ code: failure.code, kind: 'failed', retryable: failure.retryable });
		}
		if (failure.operation === undefined || failure.operation === 'selection') {
			this.#selectionGestureInFlight = false;
			this.#scheduleSelectionGesture();
		}
		if (!recoveredOverscan) {
			this.#errorEmitter.fire({ error: new Error(failure.message), operation: 'command' });
		}
	}

	clearSelection(): void {
		this.#sendSelectionCommand('clear');
	}

	selectAll(): void {
		this.#sendSelectionCommand('select-all');
	}

	copySelection(): CanonicalUint64Text {
		const requestSequence = this.#nextSideEffectSequence();
		this.#send({
			kind: 'selection-copy',
			payload: { attachmentId: this.#attachmentId, requestSequence },
		});
		return requestSequence;
	}

	copySelectionAsync(): Promise<ProjectedTerminalSideEffectResult> {
		if (this.#copyCompletions.size > 0) {
			return Promise.resolve({ code: 'copy-in-progress', kind: 'failed', retryable: true });
		}
		const requestSequence = this.#nextSideEffectSequence();
		return new Promise((resolve) => {
			this.#copyCompletions.set(requestSequence, resolve);
			this.#send({
				kind: 'selection-copy',
				payload: { attachmentId: this.#attachmentId, requestSequence },
			});
		});
	}

	openLink(target: string): CanonicalUint64Text {
		const requestSequence = this.#nextSideEffectSequence();
		this.#send({
			kind: 'link-open',
			payload: { attachmentId: this.#attachmentId, requestSequence, target },
		});
		return requestSequence;
	}

	openLinkAsync(target: string): Promise<ProjectedTerminalSideEffectResult> {
		if (this.#linkCompletions.size > 0) {
			return Promise.resolve({ code: 'link-in-progress', kind: 'failed', retryable: true });
		}
		const requestSequence = this.#nextSideEffectSequence();
		return new Promise((resolve) => {
			this.#linkCompletions.set(requestSequence, resolve);
			this.#send({
				kind: 'link-open',
				payload: { attachmentId: this.#attachmentId, requestSequence, target },
			});
		});
	}

	updateSearch(query: string, options: TerminalSearchOptions = {}): void {
		if (this.#phase === ProjectedTerminalPhase.disposed) return;
		this.#getSearchController().update(query, options);
	}

	findNext(): boolean {
		return (this.#searchController?.next(false) ?? null) !== null;
	}

	findPrevious(): boolean {
		return (this.#searchController?.next(true) ?? null) !== null;
	}

	clearSearch(): void {
		if (this.#searchController === undefined) {
			this.#renderer?.setSearchMatches([]);
			return;
		}
		this.#searchController.update('');
	}

	focus(): void {
		this.#input?.focus({ preventScroll: true });
	}

	async paste(text: string, source: ProjectedPasteSource = 'api'): Promise<ProjectedPasteResult> {
		return await this.#inputDispatcher.dispatch(() => this.#performPaste(text, source));
	}

	async #performPaste(
		text: string,
		source: ProjectedPasteSource,
	): Promise<ProjectedPasteResult> {
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
		if (this.#isDisposed() || this.#options.inputEnabled === false) {
			return { kind: 'blocked', review };
		}
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
		if (this.#isDisposed() || this.#options.inputEnabled === false) {
			return { kind: 'blocked', review };
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
		const finalRevision = BigInt(event.finalRevision);
		if (
			this.#latestExitFinalRevision !== undefined &&
			finalRevision <= this.#latestExitFinalRevision
		) {
			return;
		}
		this.#latestExitFinalRevision = finalRevision;
		this.#pendingExit = event;
		this.#finalizeExitIfCaughtUp();
	}

	dispose(): void {
		if (this.#phase === ProjectedTerminalPhase.disposed) return;
		if (this.#historyPhase !== ProjectedHistoryPhase.followingBottom) {
			this.#send({
				kind: 'set-history-demand',
				payload: { attachmentId: this.#attachmentId, demand: 'following-bottom' },
			});
		}
		if (this.#demandSent === 'active') {
			this.#demand = 'paused';
			this.#sendDemandIfNeeded();
		}
		this.#phase = ProjectedTerminalPhase.disposed;
		this.#historyPhase = ProjectedHistoryPhase.disposed;
		const rowFlight = this.#rowFlight;
		this.#rowFlight = undefined;
		if (rowFlight?.kind === 'buffer') {
			rowFlight.removeAbortListener();
			if (!rowFlight.aborted) {
				rowFlight.resolve({ ...this.getBufferRowsSnapshot(), kind: 'cancelled' });
			}
		}
		if (this.#frameHandle?.kind === 'animation-frame') {
			this.#root?.ownerDocument.defaultView?.cancelAnimationFrame(this.#frameHandle.value);
		} else if (this.#frameHandle?.kind === 'timeout') {
			globalThis.clearTimeout(this.#frameHandle.value);
		}
		this.#frameHandle = undefined;
		if (this.#selectionGestureHandle?.kind === 'animation-frame') {
			this.#root?.ownerDocument.defaultView?.cancelAnimationFrame(
				this.#selectionGestureHandle.value,
			);
		} else if (this.#selectionGestureHandle?.kind === 'timeout') {
			globalThis.clearTimeout(this.#selectionGestureHandle.value);
		}
		this.#selectionGestureHandle = undefined;
		this.#pendingHistoryDelta = 0;
		this.#pendingHistoryTargetRow = undefined;
		this.#searchController?.dispose();
		this.#searchController = undefined;
		this.#clearHoveredLink();
		for (const complete of this.#copyCompletions.values()) complete({ kind: 'cancelled' });
		for (const complete of this.#linkCompletions.values()) complete({ kind: 'cancelled' });
		this.#copyCompletions.clear();
		this.#linkCompletions.clear();
		this.#resizeObserver?.disconnect();
		this.#resizeObserver = undefined;
		this.#devicePixelRatioQuery?.removeEventListener(
			'change',
			this.#handleDevicePixelRatioChange,
		);
		this.#devicePixelRatioQuery = undefined;
		for (const dispose of this.#listenerDisposers.splice(0)) dispose();
		this.#renderer?.dispose();
		this.#renderer = undefined;
		this.#store.dispose();
		this.#historyStore?.dispose();
		this.#historyStore = undefined;
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
		this.#historyEmitter.dispose();
		this.#linkOpenEmitter.dispose();
		this.#metadataEmitter.dispose();
		this.#selectionEmitter.dispose();
		this.#selectionCopiedEmitter.dispose();
		this.#searchEmitter.dispose();
	}

	#scheduleRender(): boolean {
		if (this.#phase === ProjectedTerminalPhase.disposed || !this.#renderer) return false;
		this.#syncProjectedViewportState();
		this.#updateInputPosition();
		if (this.#frameHandle !== undefined) return true;
		const callback = (): void => {
			this.#frameHandle = undefined;
			if (this.#phase === ProjectedTerminalPhase.disposed) return;
			try {
				this.#renderer?.renderPacked(this.#visibleStore());
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

	#visibleStore(): PackedTerminalStore | PackedTerminalViewport {
		return this.#historyPhase === ProjectedHistoryPhase.followingBottom ||
			this.#historyStore === undefined
			? this.#store
			: this.#historyStore;
	}

	#syncProjectedViewportState(): void {
		const renderer = this.#renderer;
		const metadata = this.metadata;
		if (!renderer || !metadata) return;
		const absoluteTop =
			this.#historyPhase === ProjectedHistoryPhase.followingBottom
				? this.#historyTotalRows - BigInt(metadata.rows)
				: this.#historyStartRow;
		const selection = this.#selection;
		renderer.setProjectedViewportState(
			Number(absoluteTop),
			selection.state === 'active'
				? {
						endCol: selection.end.column,
						endRow: Number(BigInt(selection.end.row) - absoluteTop),
						rectangle: selection.rectangle,
						startCol: selection.start.column,
						startRow: Number(BigInt(selection.start.row) - absoluteTop),
					}
				: null,
		);
	}

	#requestHistoryRows(origin: ReadRows['origin'], overscanRows = HISTORY_OVERSCAN_ROWS): void {
		const metadata = this.#store.metadata ?? this.#historyStore?.metadata;
		if (
			metadata === null ||
			metadata === undefined ||
			this.#rowFlight !== undefined ||
			this.#phase === ProjectedTerminalPhase.disposed
		) {
			return;
		}
		const visibleRowCount = metadata.rows;
		const visibleStartRow = this.#resolveHistoryVisibleStart(origin, visibleRowCount);
		const maximumCacheRows = Math.max(
			1,
			Math.min(
				PROTOCOL_LIMITS.maxRowWindowRows,
				Math.floor(PROTOCOL_LIMITS.maxFrameCells / metadata.columns),
			),
		);
		const cacheRowCount = Math.min(
			maximumCacheRows,
			visibleRowCount + overscanRows * 2,
			Number(
				this.#historyTotalRows > BigInt(maximumCacheRows)
					? BigInt(maximumCacheRows)
					: this.#historyTotalRows,
			),
		);
		if (cacheRowCount < visibleRowCount) return;
		const maximumCacheStart =
			this.#historyTotalRows > BigInt(cacheRowCount)
				? this.#historyTotalRows - BigInt(cacheRowCount)
				: 0n;
		const preferredCacheStart =
			visibleStartRow > BigInt(overscanRows) ? visibleStartRow - BigInt(overscanRows) : 0n;
		const cacheStartRow = minBigInt(preferredCacheStart, maximumCacheStart);
		if (this.#historyPhase === ProjectedHistoryPhase.followingBottom) {
			this.#send({
				kind: 'set-history-demand',
				payload: { attachmentId: this.#attachmentId, demand: 'viewing-history' },
			});
			this.#demand = 'paused';
			this.#sendDemandIfNeeded();
		}
		this.#historyPhase = ProjectedHistoryPhase.requestingHistory;
		const requestSequence = this.#nextRowRequestSequence();
		this.#rowFlight = {
			kind: 'viewport',
			maximumBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
			overscanned: cacheRowCount > visibleRowCount,
			requestSequence,
			visibleRowCount,
			visibleStartRow,
		};
		this.#send({
			kind: 'read-rows',
			payload: {
				attachmentId: this.#attachmentId,
				detail: 'render',
				generation: CanonicalUint64TextSchema.parse(this.#historyGeneration.toString()),
				maximumBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
				origin: {
					kind: 'absolute',
					row: CanonicalUint64TextSchema.parse(cacheStartRow.toString()),
				},
				purpose: 'viewport',
				requestSequence,
				revision: CanonicalUint64TextSchema.parse(this.#historyRevision.toString()),
				rowCount: cacheRowCount,
			},
		});
	}

	#resolveHistoryVisibleStart(origin: ReadRows['origin'], visibleRows: number): bigint {
		const maximumStart =
			this.#historyTotalRows > BigInt(visibleRows)
				? this.#historyTotalRows - BigInt(visibleRows)
				: 0n;
		let start: bigint;
		switch (origin.kind) {
			case 'absolute':
				start = BigInt(origin.row);
				break;
			case 'committed':
				start = this.#historyStartRow + BigInt(origin.delta);
				break;
			case 'oldest':
				start = BigInt(origin.offset);
				break;
			case 'tail': {
				const end =
					this.#historyTotalRows - minBigInt(this.#historyTotalRows, BigInt(origin.rowsBefore));
				start = end > BigInt(visibleRows) ? end - BigInt(visibleRows) : 0n;
				break;
			}
		}
		return start < 0n ? 0n : minBigInt(start, maximumStart);
	}

	#dispatchPendingHistoryIntent(): void {
		if (this.#rowFlight !== undefined || this.#phase === ProjectedTerminalPhase.disposed) return;
		const targetRow = this.#pendingHistoryTargetRow;
		this.#pendingHistoryTargetRow = undefined;
		if (targetRow !== undefined) {
			this.#revealSearchRow(targetRow);
			return;
		}
		const pendingDelta = this.#pendingHistoryDelta;
		this.#pendingHistoryDelta = 0;
		if (pendingDelta !== 0) this.scrollLines(pendingDelta);
	}

	#recoverHistory(): void {
		if (
			this.#historyPhase === ProjectedHistoryPhase.followingBottom ||
			this.#historyPhase === ProjectedHistoryPhase.disposed
		) {
			return;
		}
		const flight = this.#rowFlight;
		this.#rowFlight = undefined;
		if (flight?.kind === 'buffer') {
			flight.removeAbortListener();
			if (!flight.aborted) {
				flight.resolve({ ...this.getBufferRowsSnapshot(), kind: 'cancelled' });
			}
		}
		this.#pendingHistoryDelta = 0;
		this.#pendingHistoryTargetRow = undefined;
		this.#searchController?.invalidate();
		this.#historyPhase = ProjectedHistoryPhase.recovering;
		this.#send({
			kind: 'set-history-demand',
			payload: { attachmentId: this.#attachmentId, demand: 'following-bottom' },
		});
		this.#demand = 'active';
		this.#sendDemandIfNeeded();
	}

	#finishHistoryRecovery(): void {
		this.#historyStore?.dispose();
		this.#historyStore = undefined;
		this.#historyRowsBefore = 0;
		this.#historyStartRow = 0n;
		this.#historyPhase = ProjectedHistoryPhase.followingBottom;
		const metadata = this.#store.metadata;
		if (metadata) {
			this.#metadataEmitter.fire(metadata);
			this.#updateAccessibility(metadata);
		}
		this.#scheduleRender();
	}

	#getSearchController(): ProjectedSearchController {
		this.#searchController ??= new ProjectedSearchController({
			createWorker: this.#options.searchWorkerFactory ?? createProjectedSearchWorker,
			onChange: (event) => {
				const active = event.activeIndex < 0 ? null : (event.matches[event.activeIndex] ?? null);
				this.#renderer?.setSearchMatches(event.matches, active);
				this.#searchEmitter.fire(event);
			},
			onReveal: (match) => this.#revealSearchMatch(match),
			reader: this,
		});
		return this.#searchController;
	}

	#revealSearchMatch(match: TerminalSearchMatch): void {
		if (!Number.isSafeInteger(match.start.row) || match.start.row < 0) return;
		this.#revealSearchRow(BigInt(match.start.row));
	}

	#revealSearchRow(row: bigint): void {
		const metadata = this.metadata ?? this.#store.metadata;
		if (metadata === null || row < 0n || row >= this.#historyTotalRows) return;
		const visibleRows = BigInt(metadata.rows);
		const visibleStart =
			this.#historyPhase === ProjectedHistoryPhase.followingBottom
				? this.#historyTotalRows - visibleRows
				: this.#historyStartRow;
		if (row >= visibleStart && row < visibleStart + visibleRows) {
			this.#scheduleRender();
			return;
		}
		if (this.#rowFlight !== undefined) {
			this.#cancelBufferFlightForViewportIntent();
			this.#pendingHistoryTargetRow = row;
			this.#pendingHistoryDelta = 0;
			return;
		}
		const maximumStart =
			this.#historyTotalRows > visibleRows ? this.#historyTotalRows - visibleRows : 0n;
		const centeredStart = row > visibleRows / 2n ? row - visibleRows / 2n : 0n;
		const startRow = centeredStart < maximumStart ? centeredStart : maximumStart;
		const rowsBefore = this.#historyTotalRows - (startRow + visibleRows);
		this.#historyRowsBefore = Number(
			rowsBefore > BigInt(Number.MAX_SAFE_INTEGER) ? BigInt(Number.MAX_SAFE_INTEGER) : rowsBefore,
		);
		this.#requestHistoryRows({
			kind: 'absolute',
			row: CanonicalUint64TextSchema.parse(startRow.toString()),
		});
	}

	#cancelBufferFlightForViewportIntent(): void {
		const flight = this.#rowFlight;
		if (flight?.kind !== 'buffer' || flight.aborted) return;
		flight.aborted = true;
		flight.resolve({ ...this.getBufferRowsSnapshot(), kind: 'cancelled' });
	}

	#queueLinkLookup(point: PendingLinkPoint): void {
		const cache = this.#linkCache;
		if (
			cache !== undefined &&
			cache.generation === this.#historyGeneration &&
			cache.revision === this.#historyRevision &&
			point.row >= cache.startRow &&
			point.row <= cache.endRow
		) {
			this.#presentHoveredLink(
				cache.links.find((link) => terminalLinkContains(link, point.column, Number(point.row))),
				point,
			);
			return;
		}
		this.#pendingLinkPoint = point;
		if (!this.#linkLookupActive) void this.#drainLinkLookups();
	}

	async #drainLinkLookups(): Promise<void> {
		this.#linkLookupActive = true;
		try {
			while (
				this.#pendingLinkPoint !== undefined &&
				this.#phase !== ProjectedTerminalPhase.disposed
			) {
				const requestedPoint = this.#pendingLinkPoint;
				this.#pendingLinkPoint = undefined;
				const snapshot = this.getBufferRowsSnapshot();
				if (requestedPoint.row >= snapshot.totalRows) {
					this.#presentHoveredLink(undefined, requestedPoint);
					continue;
				}
				const startRow = requestedPoint.row > 0n ? requestedPoint.row - 1n : 0n;
				const rowCount = Math.min(3, Number(snapshot.totalRows - startRow));
				const abortController = new AbortController();
				this.#linkAbortController = abortController;
				const result = await this.readBufferRows(
					{
						detail: 'render',
						generation: snapshot.generation,
						maximumBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
						revision: snapshot.revision,
						rowCount,
						startRow,
					},
					abortController.signal,
				);
				if (this.#linkAbortController === abortController) {
					this.#linkAbortController = undefined;
				}
				if (abortController.signal.aborted) continue;
				const currentPoint = this.#pendingLinkPoint ?? requestedPoint;
				this.#pendingLinkPoint = undefined;
				if (result.kind !== 'rows') {
					this.#presentHoveredLink(undefined, currentPoint);
					continue;
				}
				const endRow = result.startRow + BigInt(result.rows.length - 1);
				const links = detectTerminalRowLinks(result.rows, this.metadata?.currentDirectory ?? '');
				this.#linkCache = {
					endRow,
					generation: result.generation,
					links,
					revision: result.revision,
					startRow: result.startRow,
				};
				if (currentPoint.row < result.startRow || currentPoint.row > endRow) {
					this.#pendingLinkPoint = currentPoint;
					continue;
				}
				this.#presentHoveredLink(
					links.find((link) =>
						terminalLinkContains(link, currentPoint.column, Number(currentPoint.row)),
					),
					currentPoint,
				);
			}
		} finally {
			this.#linkLookupActive = false;
			if (this.#pendingLinkPoint !== undefined && this.#phase !== ProjectedTerminalPhase.disposed) {
				void this.#drainLinkLookups();
			}
		}
	}

	#presentHoveredLink(link: ILink | undefined, point: PendingLinkPoint): void {
		this.#hoveredLink = link;
		this.#hoveredLinkPoint = point;
		const enabled =
			link !== undefined &&
			(point.control || point.meta) &&
			isPermittedExternalTerminalTarget(link);
		const metadata = this.metadata;
		const renderer = this.#renderer;
		if (metadata !== null && renderer !== undefined) {
			const visibleStart =
				this.#historyPhase === ProjectedHistoryPhase.followingBottom
					? this.#historyTotalRows - BigInt(metadata.rows)
					: this.#historyStartRow;
			renderer.setHoveredLinkRange(
				enabled && link !== undefined
					? {
							endX: link.range.end.x,
							endY: link.range.end.y - Number(visibleStart),
							startX: link.range.start.x,
							startY: link.range.start.y - Number(visibleStart),
						}
					: null,
			);
		}
		if (this.#canvas !== undefined) this.#canvas.style.cursor = enabled ? 'pointer' : '';
	}

	#clearHoveredLink(): void {
		this.#pendingLinkPoint = undefined;
		this.#linkAbortController?.abort();
		this.#linkAbortController = undefined;
		this.#hoveredLink = undefined;
		this.#hoveredLinkPoint = undefined;
		this.#renderer?.setHoveredLinkRange(null);
		if (this.#canvas !== undefined) this.#canvas.style.cursor = '';
	}

	#invalidateLinkCache(): void {
		this.#linkCache = undefined;
		this.#clearHoveredLink();
	}

	#sendSelectionCommand(command: 'clear' | 'select-all'): void {
		this.#send({
			kind: 'selection-command',
			payload: {
				attachmentId: this.#attachmentId,
				command,
				generation: CanonicalUint64TextSchema.parse(this.#historyGeneration.toString()),
				revision: CanonicalUint64TextSchema.parse(this.#historyRevision.toString()),
			},
		});
	}

	#queueSelectionGesture(gesture: PendingSelectionGesture): void {
		this.#pendingSelectionGesture = gesture;
		this.#scheduleSelectionGesture();
	}

	#startSelectionGesture(gesture: PendingSelectionGesture): void {
		if (this.#selectionGestureInFlight) {
			this.#queueSelectionGesture(gesture);
			return;
		}
		this.#selectionGestureInFlight = true;
		this.#sendSelectionGesture(gesture);
	}

	#scheduleSelectionGesture(): void {
		if (
			this.#selectionGestureHandle !== undefined ||
			this.#selectionGestureInFlight ||
			this.#pendingSelectionGesture === undefined ||
			this.#phase === ProjectedTerminalPhase.disposed
		) {
			return;
		}
		const callback = (): void => {
			this.#selectionGestureHandle = undefined;
			if (this.#selectionGestureInFlight) return;
			const gesture = this.#pendingSelectionGesture;
			if (gesture === undefined || this.#phase === ProjectedTerminalPhase.disposed) return;
			this.#pendingSelectionGesture = undefined;
			this.#selectionGestureInFlight = true;
			this.#sendSelectionGesture(gesture);
		};
		const view = this.#root?.ownerDocument.defaultView;
		this.#selectionGestureHandle =
			typeof view?.requestAnimationFrame === 'function'
				? { kind: 'animation-frame', value: view.requestAnimationFrame(callback) }
				: { kind: 'timeout', value: globalThis.setTimeout(callback, 16) };
	}

	#sendSelectionGesture(gesture: PendingSelectionGesture): void {
		this.#send({
			kind: 'selection-gesture',
			payload: {
				...gesture,
				attachmentId: this.#attachmentId,
				generation: CanonicalUint64TextSchema.parse(this.#historyGeneration.toString()),
				revision: CanonicalUint64TextSchema.parse(this.#historyRevision.toString()),
			},
		});
	}

	#selectionPoint(event: MouseEvent): SelectionGesture['point'] | null {
		const canvas = this.#canvas;
		const renderer = this.#renderer;
		const metadata = this.metadata;
		if (!canvas || !renderer || !metadata) return null;
		const bounds = canvas.getBoundingClientRect();
		const column = clamp(
			Math.floor((event.clientX - bounds.left) / renderer.charWidth),
			0,
			metadata.columns - 1,
		);
		const viewportRow = clamp(
			Math.floor((event.clientY - bounds.top) / renderer.charHeight),
			0,
			metadata.rows - 1,
		);
		const startRow =
			this.#historyPhase === ProjectedHistoryPhase.followingBottom
				? this.#historyTotalRows - BigInt(metadata.rows)
				: this.#historyStartRow;
		return {
			column,
			row: CanonicalUint64TextSchema.parse((startRow + BigInt(viewportRow)).toString()),
		};
	}

	#nextRowRequestSequence(): CanonicalUint64Text {
		if (this.#rowRequestSequence > 0xffff_ffff_ffff_ffffn) this.#rowRequestSequence = 1n;
		return CanonicalUint64TextSchema.parse((this.#rowRequestSequence++).toString());
	}

	#nextSideEffectSequence(): CanonicalUint64Text {
		if (this.#sideEffectSequence > 0xffff_ffff_ffff_ffffn) this.#sideEffectSequence = 1n;
		return CanonicalUint64TextSchema.parse((this.#sideEffectSequence++).toString());
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

	#sendInput(command: ProjectedTerminalCommand): void {
		this.#inputDispatcher.dispatchSync(() => {
			if (this.#isDisposed() || this.#options.inputEnabled === false) return;
			this.#send(command);
		});
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
		if (this.#options.inputEnabled !== false) {
			this.#listen(input, 'keydown', this.#handleKeyDown);
			this.#listen(input, 'keyup', this.#handleKeyUp);
			this.#listen(input, 'beforeinput', this.#handleBeforeInput);
			this.#listen(input, 'compositionstart', this.#handleCompositionStart);
			this.#listen(input, 'compositionupdate', this.#handleCompositionUpdate);
			this.#listen(input, 'compositionend', this.#handleCompositionEnd);
			this.#listen(input, 'paste', this.#handlePaste);
		}
		this.#listen(input, 'focus', this.#handleFocus);
		this.#listen(input, 'blur', this.#handleBlur);
		this.#listen(canvas, 'pointerdown', this.#handlePointerDown);
		this.#listen(canvas, 'pointerup', this.#handlePointerUp);
		this.#listen(canvas, 'pointermove', this.#handlePointerMove);
		this.#listen(canvas, 'pointercancel', this.#handlePointerCancel);
		this.#listen(canvas, 'lostpointercapture', this.#handleLostPointerCapture);
		this.#listen(canvas, 'pointerleave', this.#handlePointerLeave);
		this.#listen(canvas, 'click', this.#handleClick);
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
		this.#refreshHoveredLinkModifiers(event);
		if (this.#composing || event.isComposing || event.keyCode === 229) return;
		if (
			event.shiftKey &&
			!event.altKey &&
			!event.ctrlKey &&
			!event.metaKey &&
			(event.code === 'PageUp' || event.code === 'PageDown')
		) {
			this.scrollPages(event.code === 'PageUp' ? -1 : 1);
			event.preventDefault();
			event.stopPropagation();
			return;
		}
		if ((event.ctrlKey || event.metaKey) && event.code === 'KeyV') return;
		const facts = this.#keyFacts(event);
		if (!facts) return;
		if (this.#historyPhase !== ProjectedHistoryPhase.followingBottom) this.scrollToBottom();
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
		this.#refreshHoveredLinkModifiers(event);
	};

	#sendKey(facts: KeyFacts, action: KeyInput['action']): void {
		this.#sendInput({
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

	#refreshHoveredLinkModifiers(event: KeyboardEvent): void {
		const point = this.#hoveredLinkPoint;
		if (point === undefined) return;
		this.#presentHoveredLink(this.#hoveredLink, {
			...point,
			control: event.ctrlKey,
			meta: event.metaKey,
		});
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
		if (this.#historyPhase !== ProjectedHistoryPhase.followingBottom) this.scrollToBottom();
		if (this.#selection.state !== 'none') this.clearSelection();
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
		this.#preeditCellWidth = 0;
		this.#preeditCaretPixelOffset = 0;
		this.#updateInputPosition();
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
		this.#preeditCellWidth = state.width;
		const caretOffset = Math.max(0, Math.min(data.length, input?.selectionEnd ?? data.length));
		this.#preeditCaretPixelOffset = data
			? (this.#renderer?.measureTextWidth(data.slice(0, caretOffset)) ??
				state.selection.end * (this.#renderer?.getMetrics().width ?? 0))
			: 0;
		this.#renderer?.setPreedit(state.cells, state.selection);
		this.#updateInputPosition();
	}

	#updateInputPosition(): void {
		const input = this.#input;
		const root = this.#root;
		const canvas = this.#canvas;
		const renderer = this.#renderer;
		const metadata = this.#store.metadata ?? this.metadata;
		if (!input || !root || !canvas || !renderer || metadata === null) return;
		const rootBounds = root.getBoundingClientRect();
		const canvasBounds = canvas.getBoundingClientRect();
		const metrics = renderer.getMetrics();
		const anchor = calculateInputAnchor({
			canvasLeft: canvasBounds.left,
			canvasTop: canvasBounds.top,
			cellHeight: metrics.height,
			cellWidth: metrics.width,
			columns: metadata.columns,
			cursorColumn: metadata.cursor.column,
			cursorRow: metadata.cursor.row,
			elementLeft: rootBounds.left,
			elementTop: rootBounds.top,
			preeditCaretPixelOffset: this.#preeditCaretPixelOffset,
			preeditCellWidth: this.#preeditCellWidth,
			rows: metadata.rows,
		});
		input.style.left = `${anchor.left}px`;
		input.style.top = `${anchor.top}px`;
		input.style.width = `${anchor.width}px`;
		input.style.height = `${anchor.height}px`;
		input.style.lineHeight = `${anchor.height}px`;
	}

	#sendText(text: string): void {
		if (textEncoder.encode(text).byteLength > PROTOCOL_LIMITS.maxInputBytes) return;
		this.#sendInput({
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
		this.#updateInputPosition();
		if (this.#options.inputEnabled === false) return;
		this.#sendInput({
			kind: 'focus-input',
			payload: { attachmentId: this.#attachmentId, focused: true },
		});
	};

	readonly #handleBlur = (): void => {
		this.#renderer?.setFocused(false);
		if (this.#options.inputEnabled === false) return;
		this.#sendInput({
			kind: 'focus-input',
			payload: { attachmentId: this.#attachmentId, focused: false },
		});
		this.#pressedKeys.clear();
		this.#pressedModifierCodes.clear();
	};

	readonly #handlePointerDown = (event: PointerEvent): void => {
		this.focus();
		if (this.#mouseOwned('press', event.buttons, event.shiftKey)) {
			this.#sendMouse(event, MouseAction.press, domMouseButton(event.button));
			event.preventDefault();
			return;
		}
		if (event.button !== 0) return;
		const hoveredLink = this.#hoveredLink;
		const hoveredPoint = this.#selectionPoint(event);
		if (
			hoveredLink !== undefined &&
			hoveredPoint !== null &&
			(event.ctrlKey || event.metaKey) &&
			terminalLinkContains(hoveredLink, hoveredPoint.column, Number(BigInt(hoveredPoint.row))) &&
			isPermittedExternalTerminalTarget(hoveredLink)
		) {
			event.preventDefault();
			return;
		}
		const point = this.#selectionPoint(event);
		if (point === null) return;
		const gesture = {
			clickCount: clamp(event.detail, 1, 3),
			point,
			rectangle: event.altKey,
		};
		this.#selectionGestureActive = true;
		this.#selectionGestureLast = gesture;
		this.#selectionGesturePointerId = event.pointerId;
		try {
			this.#canvas?.setPointerCapture(event.pointerId);
		} catch {
			// Pointer capture is best-effort; document-level cancellation still ends the gesture.
		}
		this.#startSelectionGesture({
			action: 'start',
			...gesture,
		});
		event.preventDefault();
	};

	readonly #handlePointerUp = (event: PointerEvent): void => {
		if (
			this.#selectionGestureActive &&
			event.button === 0 &&
			(this.#selectionGesturePointerId === undefined ||
				this.#selectionGesturePointerId === event.pointerId)
		) {
			const point = this.#selectionPoint(event);
			if (point !== null && this.#selectionGestureLast !== undefined) {
				this.#selectionGestureLast = {
					...this.#selectionGestureLast,
					point,
					rectangle: event.altKey,
				};
			}
			this.#finishSelectionGesture(true);
			event.preventDefault();
			return;
		}
		if (!this.#mouseOwned('release', event.buttons, event.shiftKey)) return;
		this.#sendMouse(event, MouseAction.release, domMouseButton(event.button));
		event.preventDefault();
	};

	readonly #handlePointerMove = (event: PointerEvent): void => {
		if (
			this.#selectionGestureActive &&
			(event.buttons & 1) !== 0 &&
			(this.#selectionGesturePointerId === undefined ||
				this.#selectionGesturePointerId === event.pointerId)
		) {
			this.#clearHoveredLink();
			const point = this.#selectionPoint(event);
			if (point !== null) {
				const gesture = {
					clickCount: this.#selectionGestureLast?.clickCount ?? 1,
					point,
					rectangle: event.altKey,
				};
				this.#selectionGestureLast = gesture;
				this.#queueSelectionGesture({ action: 'update', ...gesture });
			}
			event.preventDefault();
			return;
		}
		if (this.#mouseOwned('motion', event.buttons, event.shiftKey)) {
			this.#clearHoveredLink();
			this.#sendMouse(event, MouseAction.motion, null);
			event.preventDefault();
			return;
		}
		const point = this.#selectionPoint(event);
		if (point === null) {
			this.#clearHoveredLink();
			return;
		}
		this.#queueLinkLookup({
			column: point.column,
			control: event.ctrlKey,
			meta: event.metaKey,
			row: BigInt(point.row),
		});
	};

	readonly #handlePointerLeave = (): void => this.#clearHoveredLink();

	readonly #handlePointerCancel = (event: PointerEvent): void => {
		if (this.#selectionGesturePointerId !== event.pointerId) return;
		this.#finishSelectionGesture(true);
		event.preventDefault();
	};

	readonly #handleLostPointerCapture = (event: PointerEvent): void => {
		if (this.#selectionGesturePointerId !== event.pointerId) return;
		this.#finishSelectionGesture(false);
	};

	#finishSelectionGesture(releaseCapture: boolean): void {
		if (!this.#selectionGestureActive) return;
		const canvas = this.#canvas;
		const pointerId = this.#selectionGesturePointerId;
		const gesture = this.#selectionGestureLast;
		this.#selectionGestureActive = false;
		this.#selectionGestureLast = undefined;
		this.#selectionGesturePointerId = undefined;
		if (gesture !== undefined) this.#queueSelectionGesture({ action: 'end', ...gesture });
		if (!releaseCapture || pointerId === undefined) return;
		try {
			canvas?.releasePointerCapture(pointerId);
		} catch {
			// Browsers may release capture before pointerup is dispatched.
		}
	}

	readonly #handleClick = (event: MouseEvent): void => {
		if (!event.ctrlKey && !event.metaKey) return;
		const point = this.#selectionPoint(event);
		const link = this.#hoveredLink;
		if (
			point === null ||
			link === undefined ||
			!terminalLinkContains(link, point.column, Number(BigInt(point.row))) ||
			!isPermittedExternalTerminalTarget(link) ||
			link.target?.kind !== 'url'
		) {
			return;
		}
		this.#linkOpenEmitter.fire({ target: link.target });
		event.preventDefault();
	};

	readonly #handleWheel = (event: WheelEvent): void => {
		if (event.deltaY === 0) return;
		if (this.#mouseOwned('press', event.buttons, event.shiftKey)) {
			this.#sendMouse(
				event,
				MouseAction.press,
				event.deltaY < 0 ? MouseButton.four : MouseButton.five,
			);
		} else {
			const rowHeight = Math.max(1, this.#renderer?.charHeight ?? 1);
			const rows = Math.max(1, Math.round(Math.abs(event.deltaY) / rowHeight));
			this.scrollLines(event.deltaY < 0 ? -rows : rows);
		}
		event.preventDefault();
	};

	#mouseOwned(action: MouseInput['action'], buttons = 0, shift = false): boolean {
		if (this.#options.inputEnabled === false || shift) return false;
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
		const pressedButtons = domPressedButtons(event.buttons);
		const activeButton =
			action === MouseAction.motion && button === null
				? firstPressedMouseButton(pressedButtons)
				: button;
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
		this.#sendInput({
			kind: 'mouse-input',
			payload: {
				action,
				attachmentId: this.#attachmentId,
				button: activeButton,
				modifiers: this.#modifiers(event),
				pressedButtons,
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

	#installDevicePixelRatioListeners(view: Window | null): void {
		if (view === null || this.#options.renderer?.devicePixelRatio !== undefined) return;
		view.addEventListener('resize', this.#handleWindowResize);
		this.#listenerDisposers.push(() =>
			view.removeEventListener('resize', this.#handleWindowResize),
		);
		this.#watchDevicePixelRatio(view);
	}

	readonly #handleWindowResize = (): void => {
		this.#syncDevicePixelRatio();
		this.#renderer?.syncSurfacePadding();
		this.#updateInputPosition();
		this.#sendResizeIfChanged();
	};

	readonly #handleDevicePixelRatioChange = (): void => {
		this.#syncDevicePixelRatio();
	};

	#syncDevicePixelRatio(): void {
		const view = this.#root?.ownerDocument.defaultView;
		const renderer = this.#renderer;
		if (view === null || view === undefined || renderer === undefined) return;
		const ratio = view.devicePixelRatio ?? 1;
		if (!renderer.setDevicePixelRatio(ratio)) return;
		this.#devicePixelRatio = Number.isFinite(ratio) && ratio > 0 ? ratio : 1;
		this.#watchDevicePixelRatio(view);
		this.#updateInputPosition();
		this.#sendResizeIfChanged();
	}

	#watchDevicePixelRatio(view: Window): void {
		this.#devicePixelRatioQuery?.removeEventListener(
			'change',
			this.#handleDevicePixelRatioChange,
		);
		this.#devicePixelRatioQuery = undefined;
		if (typeof view.matchMedia !== 'function') return;
		const query = view.matchMedia(`(resolution: ${this.#devicePixelRatio}dppx)`);
		query.addEventListener('change', this.#handleDevicePixelRatioChange);
		this.#devicePixelRatioQuery = query;
	}

	async #loadConfiguredFont(
		document: Document,
		fontFamily: string,
		fontSize: number,
		fontWeight: number | 'bold' | 'normal',
	): Promise<void> {
		const fonts = document.fonts;
		if (!fonts || typeof fonts.load !== 'function') return;
		try {
			await fonts.load(`${fontWeight} ${fontSize}px ${fontFamily}`, 'MW你🚀');
		} catch (error) {
			if (!this.#isDisposed()) this.#errorEmitter.fire({ error, operation: 'font' });
		}
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
		root.setAttribute(
			'aria-live',
			this.#historyPhase === ProjectedHistoryPhase.followingBottom ? 'polite' : 'off',
		);
		const store = this.#visibleStore();
		while (this.#accessibilityRows.length < metadata.rows) {
			const row = root.ownerDocument.createElement('div');
			row.setAttribute('role', 'listitem');
			root.appendChild(row);
			this.#accessibilityRows.push(row);
		}
		while (this.#accessibilityRows.length > metadata.rows) {
			this.#accessibilityRows.pop()?.remove();
			this.#accessibilityColumnMappings.pop();
		}
		for (let rowIndex = 0; rowIndex < metadata.rows; rowIndex++) {
			const row = store.rows[rowIndex];
			const element = this.#accessibilityRows[rowIndex];
			if (!row || !element || (!row.dirty && !store.needsFullRedraw)) continue;
			let text = '';
			const columnMappings: number[] = [];
			for (let column = 0; column < metadata.columns; column++) {
				if (projectedCellDisplayWidth(row.widths[column] ?? 0) > 0) {
					const grapheme = row.grapheme(column) || ' ';
					text += grapheme;
					for (let offset = 0; offset < grapheme.length; offset++) {
						columnMappings.push(column);
					}
				}
			}
			const visibleText = text.trimEnd() || ' ';
			element.textContent = visibleText;
			this.#accessibilityColumnMappings[rowIndex] = columnMappings.slice(0, visibleText.length);
			const absoluteRow =
				this.#historyPhase === ProjectedHistoryPhase.followingBottom
					? this.#historyTotalRows - BigInt(metadata.rows) + BigInt(rowIndex)
					: this.#historyStartRow + BigInt(rowIndex);
			element.setAttribute('aria-posinset', (absoluteRow + 1n).toString());
			element.setAttribute('aria-setsize', this.#historyTotalRows.toString());
			element.setAttribute('data-terminal-absolute-row', absoluteRow.toString());
		}
		const visibleStart =
			this.#historyPhase === ProjectedHistoryPhase.followingBottom
				? this.#historyTotalRows - BigInt(metadata.rows)
				: this.#historyStartRow;
		const visibleEnd =
			this.#historyTotalRows === 0n
				? 0n
				: minBigInt(this.#historyTotalRows, visibleStart + BigInt(metadata.rows));
		const historyRange = this.#options.labels.historyRange;
		if (historyRange !== undefined) {
			root.setAttribute(
				'aria-description',
				historyRange(
					this.#historyTotalRows === 0n ? 0n : visibleStart + 1n,
					visibleEnd,
					this.#historyTotalRows,
				),
			);
		}
		root.setAttribute('aria-rowcount', this.#historyTotalRows.toString());
		root.setAttribute(
			'data-terminal-visible-row-start',
			(this.#historyPhase === ProjectedHistoryPhase.followingBottom
				? this.#historyTotalRows - BigInt(metadata.rows)
				: this.#historyStartRow
			).toString(),
		);
		root.setAttribute('data-terminal-retained-rows', this.#historyTotalRows.toString());
	}

	readonly #handleAccessibilitySelectionChange = (): void => {
		if (
			this.#phase === ProjectedTerminalPhase.disposed ||
			this.#selectionGestureInFlight ||
			this.#selectionGestureActive
		) {
			return;
		}
		const selection = this.#root?.ownerDocument.getSelection();
		if (
			selection === undefined ||
			selection === null ||
			selection.rangeCount !== 1 ||
			selection.isCollapsed
		) {
			return;
		}
		const range = selection.getRangeAt(0);
		const start = this.#accessibilitySelectionPoint(
			range.startContainer,
			range.startOffset,
			'start',
		);
		const end = this.#accessibilitySelectionPoint(range.endContainer, range.endOffset, 'end');
		if (start === null || end === null) return;
		this.#startSelectionGesture({
			action: 'start',
			clickCount: 1,
			point: start,
			rectangle: false,
		});
		this.#queueSelectionGesture({
			action: 'end',
			clickCount: 1,
			point: end,
			rectangle: false,
		});
	};

	#accessibilitySelectionPoint(
		node: Node,
		offset: number,
		edge: 'end' | 'start',
	): SelectionGesture['point'] | null {
		const element = isElementNode(node) ? node : node.parentElement;
		if (element === null) return null;
		const rowElement = element.closest('[data-terminal-absolute-row]');
		if (rowElement === null || !this.#accessibilityRoot?.contains(rowElement)) {
			return null;
		}
		const rowIndex = this.#accessibilityRows.indexOf(rowElement);
		const mapping = this.#accessibilityColumnMappings[rowIndex];
		const absoluteRow = rowElement.getAttribute('data-terminal-absolute-row');
		if (mapping === undefined || mapping.length === 0 || absoluteRow === null) return null;
		const mappingOffset = clamp(edge === 'end' ? offset - 1 : offset, 0, mapping.length - 1);
		return {
			column: mapping[mappingOffset] ?? 0,
			row: CanonicalUint64TextSchema.parse(absoluteRow),
		};
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

function isElementNode(node: Node): node is Element {
	return node.nodeType === node.ELEMENT_NODE;
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
	if ((buttons & 32) !== 0) normalized |= 1 << (MouseButton.six - 1);
	if ((buttons & 64) !== 0) normalized |= 1 << (MouseButton.seven - 1);
	if ((buttons & 128) !== 0) normalized |= 1 << (MouseButton.eight - 1);
	if ((buttons & 256) !== 0) normalized |= 1 << (MouseButton.nine - 1);
	if ((buttons & 512) !== 0) normalized |= 1 << (MouseButton.ten - 1);
	if ((buttons & 1_024) !== 0) normalized |= 1 << (MouseButton.eleven - 1);
	return normalized;
}

function firstPressedMouseButton(buttons: number): MouseInput['button'] {
	for (const button of [
		MouseButton.left,
		MouseButton.right,
		MouseButton.middle,
		MouseButton.four,
		MouseButton.five,
		MouseButton.six,
		MouseButton.seven,
		MouseButton.eight,
		MouseButton.nine,
		MouseButton.ten,
		MouseButton.eleven,
	]) {
		if ((buttons & (1 << (button - 1))) !== 0) return button;
	}
	return null;
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

function minBigInt(left: bigint, right: bigint): bigint {
	return left < right ? left : right;
}

function projectedNow(): number {
	return typeof performance !== 'undefined' && typeof performance.now === 'function'
		? performance.now()
		: Date.now();
}
