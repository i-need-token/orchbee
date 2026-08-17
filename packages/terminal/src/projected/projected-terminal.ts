import {
	ActiveScreen,
	KeyAction,
	CanonicalUint64TextSchema,
	decodeTerminalRowWindow,
	MouseAction,
	MouseButton,
	PROTOCOL_LIMITS,
	SelectionPointSchema,
	TERMINAL_ROW_WINDOW_HEADER_BYTES,
	TerminalFrameKind,
	TerminalKeyCode,
	viewTerminalRowWindow,
	type AttachmentId,
	type CanonicalUint64Text,
	type CorrelationId,
	type ClearTerminal,
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
	type ResetTerminal,
	type ResizeTerminal,
	type RowsApplied,
	type SelectionChanged,
	type SelectionCommand,
	type SelectionGesture,
	type SelectionPoint,
	type SetHistoryDemand,
	type SetProjectionDemand,
	type TerminalBell,
	type TerminalClipboardWriteRequest,
	type TerminalExited,
	type TerminalId,
	type TerminalKeyCodeValue,
	type TerminalModifiers,
	type TextInput,
} from '@orchbee/terminal-protocol';
import { eastAsianWidth } from 'get-east-asian-width';

import { EventEmitter, type IEvent } from '../EventEmitter';
import {
	AccessibleTextModel,
	buildAccessibleRow,
	createAccessibilityAnnouncement,
	detectAccessibilityDeletion,
	type AccessibleRow,
} from '../accessibility';
import {
	bufferRowsFromDecodedWindow,
	validateBufferRowsRequest,
	type BufferRowsResult,
	type TerminalBufferRow,
	type TerminalBufferRows,
	type TerminalBufferRowsRequest,
	type TerminalBufferRowsSnapshot,
} from '../buffer-rows';
import {
	beginTerminalComposition,
	buildPreeditState,
	calculateInputAnchor,
	classifySurfaceMousePlatform,
	createTerminalInputState,
	disposeTerminalInputState,
	finishTerminalComposition,
	ghosttyCodepointWidth,
	observeTerminalBeforeInput,
	observeTerminalPaste,
	OrderedInputDispatcher,
	projectPreeditLayout,
	recordTerminalKeyInput,
	recordTerminalMouseSample,
	recordTerminalPaste,
	repairTerminalInputState,
	resolveSurfaceMouseCursor,
	supersedeCancelledTerminalComposition,
	TerminalBeforeInputOrigin,
	shouldSuppressComposingControlInput,
	TerminalCompositionPhase,
	terminalCompositionUpdateIsEligible,
	type PreeditState,
	type SurfaceMouseModifiers,
	type TerminalInputState,
} from '../input';
import {
	beginHistoryScrollbarDrag,
	HISTORY_SCROLLBAR_FADE_DELAY_MS,
	historyScrollbarContainsPoint,
	projectHistoryScrollbar,
	resolveHistoryScrollbarDrag,
	type HistoryScrollbarDrag,
	type HistoryScrollbarProjection,
} from '../history-scrollbar';
import {
	type RendererFrameTelemetry,
	type RendererHealthEvent,
	type PreparedTerminalPresentationConfiguration,
	WebGPURenderer,
} from '../rendering';
import {
	normalizeTerminalPresentationConfiguration,
	presentationConfigurationFromRendererOptions,
	type TerminalPresentationConfiguration,
	type TerminalPresentationConfigurationUpdateResult,
} from '../presentation-configuration';
import {
	createProjectedSearchWorker,
	ProjectedSearchController,
	type ProjectedSearchWorkerFactory,
	type ProjectedTerminalSearchEvent,
	type TerminalSearchMatch,
	type TerminalSearchOptions,
} from '../search';
import { calculateSurfaceGrid } from '../surface-size';
import {
	detectTerminalRowLinks,
	isPermittedExternalTerminalTarget,
	terminalLinkContains,
} from '../terminal-row-links';
import type { ILink, RendererOptions, SurfacePadding } from '../types';
import {
	PackedTerminalStore,
	PackedTerminalViewport,
	projectedCellDisplayWidth,
	type ProjectedTerminalMetadata,
} from './packed-terminal-store';

const terminalKeyCodes: ReadonlyMap<string, TerminalKeyCodeValue> = new Map(
	TerminalKeyCode.map((code) => [code, code]),
);
const LEGACY_KEY_CODE_ALIASES: ReadonlyMap<string, TerminalKeyCodeValue> = new Map([
	['OSLeft', 'MetaLeft'],
	['OSRight', 'MetaRight'],
]);
const W3C_KEY_STRING_PATTERN = /^\P{Cc}\p{M}*$/u;
const textEncoder = new TextEncoder();
const INPUT_DEDUPLICATION_MS = 100;
const BRACKETED_PASTE_END = '\u001b[201~';
const MAX_PTY_PIXEL_DIMENSION = 0xffff;
const MAX_HISTORY_CACHE_BYTES = 8 * 1024 * 1024;
const HISTORY_OVERSCAN_ROWS = 16;
const LINK_LOOKUP_INITIAL_PAGE_ROWS = 16;
const SELECTION_AUTOSCROLL_INTERVAL_MS = 16;
const SELECTION_CLICK_REPEAT_INTERVAL_MS = 500;
const MAX_PENDING_SELECTION_GESTURES = 64;
const RESIZE_REENTRY_MS = 50;
const TERMINAL_FONT_LOAD_SAMPLE = 'MW你🚀\uF120';

function cssPixels(value: string): number {
	const parsed = Number.parseFloat(value);
	return Number.isFinite(parsed) ? Math.max(0, parsed) : 0;
}

function isTextInsertionInputType(inputType: string): boolean {
	return (
		inputType === 'insertText' ||
		inputType === 'insertReplacementText' ||
		inputType === 'insertCompositionText' ||
		inputType === 'insertFromComposition'
	);
}

function isCompositionBeforeInput(event: InputEvent): boolean {
	return (
		event.isComposing ||
		event.inputType === 'insertCompositionText' ||
		event.inputType === 'insertFromComposition'
	);
}

function clipboardContainsImage(clipboardData: DataTransfer | null): boolean {
	const items = clipboardData?.items;
	if (items === undefined) return false;
	for (let index = 0; index < items.length; index++) {
		const item = items[index];
		if (item?.kind === 'file' && item.type.toLowerCase().startsWith('image/')) return true;
	}
	return false;
}

function readSurfacePadding(root: HTMLElement): SurfacePadding {
	const view = root.ownerDocument.defaultView;
	if (view === null) return { bottom: 0, left: 0, right: 0, top: 0 };
	const style = view.getComputedStyle(root);
	return {
		bottom: cssPixels(style.paddingBottom),
		left: cssPixels(style.paddingLeft),
		right: cssPixels(style.paddingRight),
		top: cssPixels(style.paddingTop),
	};
}

export type ProjectedTerminalCommand =
	| { readonly kind: 'clear-terminal'; readonly payload: ClearTerminal }
	| { readonly kind: 'focus-input'; readonly payload: FocusInput }
	| { readonly kind: 'frame-applied'; readonly payload: FrameApplied }
	| { readonly kind: 'key-input'; readonly payload: KeyInput }
	| { readonly kind: 'mouse-input'; readonly payload: MouseInput }
	| { readonly kind: 'need-snapshot'; readonly payload: NeedSnapshot }
	| { readonly kind: 'paste-input'; readonly payload: PasteInput }
	| { readonly kind: 'read-rows'; readonly payload: ReadRows }
	| { readonly kind: 'resize-terminal'; readonly payload: ResizeTerminal }
	| { readonly kind: 'reset-terminal'; readonly payload: ResetTerminal }
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
	/** Localized deletion announcement for assistive technology. */
	readonly deleted?: (text: string) => string;
	/** Localized current/retained row-range description for assistive technology. */
	readonly historyRange?: (start: bigint, end: bigint, total: bigint) => string;
	/** Localized label for the hidden keyboard/IME input. */
	readonly input: string;
	/** Localized label for one absolute terminal row; row is one-based. */
	readonly row?: (row: bigint, text: string) => string;
	/** Localized label for the visible-viewport accessibility mirror. */
	readonly screen: string;
	/** Localized screen label with a one-based absolute cursor position. */
	readonly screenCursor?: (row: bigint, column: number) => string;
	/** Localized description of the bounded Host selection preview. */
	readonly selectedText?: (text: string) => string;
}

export interface ProjectedPasteReview {
	readonly bracketed: boolean;
	readonly embeddedBracketTerminator: boolean;
	readonly multiline: boolean;
	readonly originalText: string;
	readonly replacedControlCharacters: number;
	readonly source: ProjectedPasteSource;
	/** Sanitized text that will be sent after approval. */
	readonly text: string;
}

export type ProjectedPasteSource = 'api' | 'beforeinput' | 'clipboard' | 'drop' | 'file-drop';

export type ProjectedPasteResult =
	| { readonly kind: 'blocked'; readonly review: ProjectedPasteReview }
	| { readonly kind: 'empty' }
	| { readonly kind: 'sent'; readonly review: ProjectedPasteReview };

export interface ProjectedTerminalOptions {
	readonly attachmentId: AttachmentId;
	readonly autoActivate?: boolean;
	readonly canResize?: boolean;
	/** Copy a completed pointer or semantic-output selection, matching the legacy surface. */
	readonly copyOnSelect?: boolean;
	readonly creationId: CreationId;
	readonly hostEpoch: HostEpoch;
	readonly inputEnabled?: boolean;
	readonly labels: ProjectedTerminalLabels;
	readonly port: ProjectedTerminalCommandPort;
	/** Trust boundary for OBPB 0.0.1 render buffers. Omitted means strict external validation. */
	readonly projectionBufferTrust?: 'external' | 'local-host';
	readonly renderer?: RendererOptions;
	readonly reviewPaste?: (
		review: ProjectedPasteReview,
		signal: AbortSignal,
	) => boolean | Promise<boolean>;
	readonly searchWorkerFactory?: ProjectedSearchWorkerFactory;
	readonly terminalId: TerminalId;
}

export type ProjectedTerminalErrorEvent =
	| {
			readonly error: unknown;
			readonly operation:
				| 'bell'
				| 'clipboard'
				| 'command'
				| 'font'
				| 'frame'
				| 'history'
				| 'open'
				| 'render'
				| 'resize'
				| 'selection';
	  }
	| (ProjectedBrokerFailure & { readonly error: Error });

export interface ProjectedBrokerFailure {
	readonly code: string;
	readonly correlationId?: CorrelationId | undefined;
	readonly message: string;
	readonly operation?:
		| 'clipboard'
		| 'connection'
		| 'control'
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

export interface ProjectedTerminalSelectionPoint {
	readonly column: number;
	readonly row: bigint;
}

export interface ProjectedTerminalSelectionRange {
	readonly end: ProjectedTerminalSelectionPoint;
	readonly rectangle: boolean;
	readonly start: ProjectedTerminalSelectionPoint;
}

export type ProjectedTerminalSideEffectResult =
	| { readonly byteLength?: number; readonly kind: 'completed' }
	| { readonly code: string; readonly kind: 'failed'; readonly retryable: boolean }
	| { readonly kind: 'cancelled' };

export type ProjectedTerminalSelectionCopyResult = Exclude<
	ProjectedTerminalSideEffectResult,
	{ readonly kind: 'cancelled' }
>;

export interface ProjectedTerminalSelectionCopyEvent {
	readonly requestSequence: CanonicalUint64Text;
	readonly result: ProjectedTerminalSelectionCopyResult;
	readonly source: 'automatic' | 'manual';
}

export interface ProjectedTerminalFrameTelemetry extends RendererFrameTelemetry {
	readonly generation: CanonicalUint64Text;
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
	frozen: 'frozen',
	open: 'open',
	opening: 'opening',
} as const;

export type ProjectedTerminalPhase =
	(typeof ProjectedTerminalPhase)[keyof typeof ProjectedTerminalPhase];

const ProjectedInputAdmission = {
	accepting: 'accepting',
	disposed: 'disposed',
	repairing: 'repairing',
} as const;

type ProjectedInputAdmission =
	(typeof ProjectedInputAdmission)[keyof typeof ProjectedInputAdmission];

const ProjectedInputPublicationIntent = {
	held: 'held',
	stateless: 'stateless',
} as const;

type ProjectedInputPublicationIntent =
	(typeof ProjectedInputPublicationIntent)[keyof typeof ProjectedInputPublicationIntent];

interface ProjectedInputRepairTransaction {
	afterCleanup: (() => void) | undefined;
	readonly attemptedKeys: Set<TerminalKeyCodeValue>;
	readonly attemptedMouseButtons: Set<Exclude<MouseInput['button'], null>>;
	readonly commands: ProjectedTerminalCommand[];
	deferredDispose: boolean;
	deferredFocusLoss: boolean;
	dispatchRequested: boolean;
	focusOutCommand: ProjectedTerminalCommand | undefined;
	focusOutSent: boolean;
	nextCommandIndex: number;
	transientDisposed: boolean;
	transientRepaired: boolean;
}

interface KeyFacts {
	readonly composing: boolean;
	readonly consumedModifiers: TerminalModifiers;
	readonly key: TerminalKeyCodeValue;
	readonly modifiers: TerminalModifiers;
	readonly text: string;
	readonly unshiftedCodepoint: number;
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
	readonly expectedStartRow: bigint | null;
	readonly kind: 'viewport';
	readonly maximumBytes: number;
	readonly origin: ReadRows['origin'];
	readonly overscanned: boolean;
	readonly requestSequence: CanonicalUint64Text;
	readonly visibleRowCount: number;
}

type RowFlight = BufferRowFlight | ViewportRowFlight;

type PendingHistoryTransition = 'awaiting-change' | 'recover' | 'refresh';

type SelectionGestureIdentity = 'attachmentId' | 'generation' | 'revision';
type PendingSelectionGesture =
	| Omit<Extract<SelectionGesture, { readonly action: 'start' }>, SelectionGestureIdentity>
	| Omit<Extract<SelectionGesture, { readonly action: 'update' }>, SelectionGestureIdentity>
	| Omit<Extract<SelectionGesture, { readonly action: 'end' }>, SelectionGestureIdentity>
	| Omit<Extract<SelectionGesture, { readonly action: 'cancel' }>, SelectionGestureIdentity>;

interface SelectionGestureLocation {
	readonly geometry: Extract<SelectionGesture, { readonly action: 'update' }>['geometry'];
	readonly point: SelectionPoint;
	readonly position: Extract<SelectionGesture, { readonly action: 'start' }>['position'];
	readonly surfaceX: number;
	readonly surfaceY: number;
}

interface SelectionClickSequence {
	readonly clickCount: number;
	readonly generation: bigint;
	readonly surfaceX: number;
	readonly surfaceY: number;
	readonly timeStamp: number;
}

interface PendingSelectionPointer {
	readonly clientX: number;
	readonly clientY: number;
	readonly rectangle: boolean;
}

interface PendingLinkPoint {
	readonly column: number;
	readonly control: boolean;
	readonly generation: bigint;
	readonly linkEpoch: bigint;
	readonly meta: boolean;
	readonly revision: bigint;
	readonly row: bigint;
}

interface HistoryScrollbarGesture {
	readonly drag: HistoryScrollbarDrag;
	readonly pointerId: number;
}

interface ApplicationMouseGesture {
	readonly pointerId: number;
}

interface PointerClickSuppression {
	readonly clientX: number;
	readonly clientY: number;
}

interface ProjectedLinkCache {
	readonly endRow: bigint;
	readonly generation: bigint;
	readonly links: readonly ILink[];
	readonly revision: bigint;
	readonly startRow: bigint;
}

interface ProjectedLinkLogicalLine {
	readonly completeAfter: boolean;
	readonly completeBefore: boolean;
	readonly rows: readonly TerminalBufferRow[];
}

interface ProjectedLinkLookup {
	readonly generation: bigint;
	readonly revision: bigint;
	readonly rows: readonly TerminalBufferRow[];
}

interface ProjectedLinkWindow {
	direction: 'backward' | 'forward' | undefined;
	pageRows: number;
	remainingBytes: number;
	readonly rows: TerminalBufferRow[];
}

type ScheduledFrame =
	| { readonly kind: 'animation-frame'; readonly value: number }
	| { readonly kind: 'timeout'; readonly value: ReturnType<typeof setTimeout> };

interface PresentationPreparation {
	readonly controller: AbortController;
	readonly epoch: number;
}

interface PreparedPaste {
	readonly review: ProjectedPasteReview;
	readonly sanitized: string;
}

type PresentationFontPreparation =
	| { readonly kind: 'ready' }
	| { readonly error: unknown; readonly kind: 'failed' }
	| { readonly kind: 'cancelled'; readonly reason: 'disposed' | 'superseded' };

function logicalLineInWindow(
	rows: readonly TerminalBufferRow[],
	targetRow: bigint,
): ProjectedLinkLogicalLine | undefined {
	const targetIndex = rows.findIndex((row) => row.index === targetRow);
	if (targetIndex < 0) return undefined;
	let startIndex = targetIndex;
	while (startIndex > 0 && rows[startIndex]?.wrapContinuation === true) {
		const previous = rows[startIndex - 1];
		const current = rows[startIndex];
		if (
			previous === undefined ||
			current === undefined ||
			previous.index + 1n !== current.index ||
			!previous.wrap
		) {
			return undefined;
		}
		startIndex -= 1;
	}
	let endIndex = targetIndex;
	while (endIndex + 1 < rows.length && rows[endIndex]?.wrap === true) {
		const current = rows[endIndex];
		const next = rows[endIndex + 1];
		if (
			current === undefined ||
			next === undefined ||
			current.index + 1n !== next.index ||
			!next.wrapContinuation
		) {
			return undefined;
		}
		endIndex += 1;
	}
	const logicalRows = rows.slice(startIndex, endIndex + 1);
	return {
		completeAfter: logicalRows.at(-1)?.wrap === false,
		completeBefore: logicalRows[0]?.wrapContinuation === false,
		rows: logicalRows,
	};
}

export class ProjectedTerminal {
	readonly #accessibilityModel = new AccessibleTextModel();
	readonly #accessibilityModelRows: AccessibleRow[] = [];
	readonly #accessibilityRows: Element[] = [];
	readonly #attachmentId: AttachmentId;
	readonly #bellEmitter = new EventEmitter<TerminalBell>();
	readonly #clipboardWriteRequestEmitter = new EventEmitter<TerminalClipboardWriteRequest>();
	readonly #creationId: CreationId;
	readonly #copyCompletions = new Map<
		CanonicalUint64Text,
		(result: ProjectedTerminalSideEffectResult) => void
	>();
	readonly #copyRequests = new Map<
		CanonicalUint64Text,
		ProjectedTerminalSelectionCopyEvent['source']
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
	readonly #deferredKeyReleases = new Map<TerminalKeyCodeValue, KeyFacts>();
	readonly #pressedKeys = new Map<string, KeyFacts>();
	readonly #pressedModifierCodes = new Set<string>();
	readonly #pressedMouseButtons = new Set<Exclude<MouseInput['button'], null>>();
	readonly #publishedKeys = new Map<TerminalKeyCodeValue, KeyFacts>();
	readonly #publishedMouseButtons = new Set<Exclude<MouseInput['button'], null>>();
	readonly #store: PackedTerminalStore;
	readonly #selectionEmitter = new EventEmitter<SelectionChanged>();
	readonly #selectionCopyResultEmitter = new EventEmitter<ProjectedTerminalSelectionCopyEvent>();
	readonly #selectionCopiedEmitter = new EventEmitter<{
		readonly byteLength: number;
		readonly requestSequence: CanonicalUint64Text;
	}>();
	readonly #searchEmitter = new EventEmitter<ProjectedTerminalSearchEvent>();
	readonly #terminalId: TerminalId;
	#accessibilityAnnouncement: HTMLDivElement | undefined;
	#accessibilityAppliedOffsets: { readonly end: number; readonly start: number } | undefined;
	#accessibilityOwnsDomSelection = false;
	#accessibilityPreviousCursor: { readonly column: number; readonly row: number } | undefined;
	#accessibilityPreviousModelRows: readonly AccessibleRow[] = [];
	#accessibilityPreviousRows: readonly string[] = [];
	#accessibilityRoot: HTMLDivElement | undefined;
	#accessibilityVisibleStart = 0n;
	#applicationMouseClickSuppression: PointerClickSuppression | undefined;
	#applicationMouseClickSuppressionTimer: ReturnType<typeof setTimeout> | undefined;
	#applicationMouseGesture: ApplicationMouseGesture | undefined;
	#canvas: HTMLCanvasElement | undefined;
	#copyOnSelect: boolean;
	#demand: ProjectionDemand = 'active';
	#demandSent: ProjectionDemand | undefined;
	#devicePixelRatio = 1;
	#devicePixelRatioQuery: MediaQueryList | undefined;
	#documentHidden = false;
	#frameHandle: ScheduledFrame | undefined;
	#historyGeneration = 1n;
	#historyPhase: ProjectedHistoryPhase = ProjectedHistoryPhase.followingBottom;
	#historyRevision = 0n;
	#historyScrollbarClickSuppression: PointerClickSuppression | undefined;
	#historyScrollbarClickSuppressionTimer: ReturnType<typeof setTimeout> | undefined;
	#historyScrollbarFadeTimer: ReturnType<typeof setTimeout> | undefined;
	#historyScrollbarGesture: HistoryScrollbarGesture | undefined;
	#historyScrollbarOpacity = 0;
	#historyScrollbar: HTMLDivElement | undefined;
	#historyScrollbarThumb: HTMLDivElement | undefined;
	#historyStartRow = 0n;
	#historyStore: PackedTerminalViewport | undefined;
	#historyTotalRows = 0n;
	#historyRowsBefore = 0;
	#inputEnabled: boolean;
	#input: HTMLTextAreaElement | undefined;
	#inputAdmission: ProjectedInputAdmission = ProjectedInputAdmission.accepting;
	#inputFocused = false;
	#inputPublicationActive = false;
	#inputRefocusAfterRepair = false;
	#inputRepairDraining = false;
	#inputRepairEpoch = 0;
	#inputRepairTransaction: ProjectedInputRepairTransaction | undefined;
	#inputState: TerminalInputState<MouseInput> = createTerminalInputState();
	#lastGeometry: ResizeGeometry | undefined;
	#lastGeometryAttemptedAt = Number.NEGATIVE_INFINITY;
	#listenerDisposers: Array<() => void> = [];
	#hoveredLink: ILink | undefined;
	#hoveredLinkPoint: PendingLinkPoint | undefined;
	#linkAbortController: AbortController | undefined;
	#linkCache: ProjectedLinkCache | undefined;
	#linkEpoch = 0n;
	#linkLookupActive = false;
	#pendingLinkPoint: PendingLinkPoint | undefined;
	#pendingExit: TerminalExited | undefined;
	#latestExitFinalRevision: bigint | undefined;
	#terminalExited = false;
	#latestBellSequence = 0n;
	#latestClipboardWriteSequence = 0n;
	#pendingAlternateWheelRows = 0;
	#pendingApplicationWheelX = 0;
	#pendingApplicationWheelY = 0;
	#pendingHistoryDelta = 0;
	#pendingHistoryTransition: PendingHistoryTransition | undefined;
	#pendingHistoryStartRow: bigint | undefined;
	#pendingHistoryTargetRow: bigint | undefined;
	#pendingHistoryWheelRows = 0;
	#pendingSelectionGestures: PendingSelectionGesture[] = [];
	#ownsResizeLease: boolean;
	#phase: ProjectedTerminalPhase = ProjectedTerminalPhase.constructed;
	#disposeAfterRepair = false;
	#disposalStarted = false;
	#presentationConfiguration: TerminalPresentationConfiguration;
	#presentationPaused = false;
	#presentationPreparation: PresentationPreparation | undefined;
	#presentationRequestEpoch = 0;
	#publishedInputFocused = false;
	#publishedMouseSample: MouseInput | undefined;
	#presentationSuspended = false;
	#preeditCaretPixelOffset = 0;
	#preeditCaretUtf16Offset = 0;
	#preeditCellWidth = 0;
	#preeditPresented = false;
	#preeditState: PreeditState | undefined;
	#preeditText = '';
	#renderer: WebGPURenderer | undefined;
	#resizeObserver: ResizeObserver | undefined;
	#root: HTMLElement | undefined;
	#rowFlight: RowFlight | undefined;
	#rowRequestSequence = 1n;
	#selection: SelectionChanged;
	#selectionGestureHandle: ScheduledFrame | undefined;
	#selectionGestureActionInFlight: SelectionGesture['action'] | undefined;
	#selectionGestureInFlight = false;
	#selectionGestureActive = false;
	#selectionGestureLast: SelectionPoint | undefined;
	#selectionGesturePointerId: number | undefined;
	#selectionClickSequence: SelectionClickSequence | undefined;
	#selectionAutoscrollDirection: 'down' | 'up' | undefined;
	#selectionAutoscrollInterval: ReturnType<typeof setInterval> | undefined;
	#selectionPointer: PendingSelectionPointer | undefined;
	#suppressNextSelectionCopy = false;
	#searchController: ProjectedSearchController | undefined;
	#searchOwnedSelection: TerminalSearchMatch | null = null;
	#sideEffectSequence = 1n;
	#surfaceMouseModifiers: SurfaceMouseModifiers = {
		alt: false,
		control: false,
		meta: false,
		shift: false,
	};
	readonly #surfaceMousePlatform = classifySurfaceMousePlatform(
		globalThis.navigator?.platform ?? '',
	);
	#synchronizingAccessibilitySelection = false;

	constructor(options: ProjectedTerminalOptions) {
		this.#options = options;
		this.#hostEpoch = options.hostEpoch;
		this.#attachmentId = options.attachmentId;
		this.#creationId = options.creationId;
		this.#store = new PackedTerminalStore(options.hostEpoch, options.attachmentId);
		this.#terminalId = options.terminalId;
		this.#inputEnabled = options.inputEnabled !== false;
		this.#selection = { attachmentId: options.attachmentId, state: 'none' };
		this.#copyOnSelect = options.copyOnSelect ?? true;
		this.#ownsResizeLease = options.canResize === true;
		this.#presentationConfiguration = presentationConfigurationFromRendererOptions(
			options.renderer,
		);
		if (options.autoActivate === false) {
			this.#demand = 'paused';
			this.#presentationPaused = true;
		}
	}

	get phase(): ProjectedTerminalPhase {
		return this.#phase;
	}

	get metadata(): ProjectedTerminalMetadata | null {
		const metadata = this.#visibleStore().metadata;
		return metadata === null ? null : this.#metadataWithLiveCloseActivity(metadata);
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

	get canControlTerminal(): boolean {
		return !this.#isDisposed() && this.#inputEnabled;
	}

	get copyOnSelect(): boolean {
		return this.#copyOnSelect;
	}

	readonly onError: IEvent<ProjectedTerminalErrorEvent> = this.#errorEmitter.event;
	readonly onBell: IEvent<TerminalBell> = this.#bellEmitter.event;
	readonly onClipboardWriteRequest: IEvent<TerminalClipboardWriteRequest> =
		this.#clipboardWriteRequestEmitter.event;
	readonly onExit: IEvent<TerminalExited> = this.#exitEmitter.event;
	readonly onFrameTelemetry: IEvent<ProjectedTerminalFrameTelemetry> =
		this.#frameTelemetryEmitter.event;
	readonly onMetadataChange: IEvent<ProjectedTerminalMetadata> = this.#metadataEmitter.event;
	readonly onRendererHealth: IEvent<RendererHealthEvent> = this.#healthEmitter.event;
	readonly onHistoryChange: IEvent<HistoryChanged> = this.#historyEmitter.event;
	readonly onLinkOpenRequest: IEvent<ProjectedTerminalLinkOpenRequest> =
		this.#linkOpenEmitter.event;
	readonly onSelectionChange: IEvent<SelectionChanged> = this.#selectionEmitter.event;
	readonly onSelectionCopyResult: IEvent<ProjectedTerminalSelectionCopyEvent> =
		this.#selectionCopyResultEmitter.event;
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
			this.#documentHidden = document.visibilityState === 'hidden';
			this.#devicePixelRatio =
				this.#options.renderer?.devicePixelRatio ?? view?.devicePixelRatio ?? 1;
			const presentationConfiguration = this.#presentationConfiguration;
			try {
				await this.#loadConfiguredFonts(document, presentationConfiguration);
			} catch (error) {
				if (!this.#isDisposed()) this.#errorEmitter.fire({ error, operation: 'font' });
			}
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
			if (!this.#inputEnabled) input.setAttribute('readonly', '');
			input.style.position = 'absolute';
			input.style.width = '1px';
			input.style.height = '1px';
			input.style.padding = '0';
			input.style.border = '0';
			input.style.margin = '0px';
			input.style.opacity = '0';
			input.style.clipPath = 'inset(50%)';
			input.style.overflow = 'hidden';
			input.style.whiteSpace = 'nowrap';
			input.style.resize = 'none';
			input.style.color = 'transparent';
			input.style.background = 'transparent';
			input.style.caretColor = 'transparent';
			this.#applyInputPresentation(input, presentationConfiguration);
			root.appendChild(input);
			this.#input = input;

			const accessibilityRoot = document.createElement('div');
			accessibilityRoot.setAttribute('aria-label', this.#options.labels.screen);
			accessibilityRoot.setAttribute('aria-live', 'off');
			accessibilityRoot.setAttribute('role', 'list');
			accessibilityRoot.style.position = 'absolute';
			accessibilityRoot.style.width = '1px';
			accessibilityRoot.style.height = '1px';
			accessibilityRoot.style.margin = '-1px';
			accessibilityRoot.style.padding = '0';
			accessibilityRoot.style.border = '0';
			accessibilityRoot.style.overflow = 'hidden';
			accessibilityRoot.style.clipPath = 'inset(50%)';
			accessibilityRoot.style.whiteSpace = 'pre';
			accessibilityRoot.style.userSelect = 'text';
			root.appendChild(accessibilityRoot);
			this.#accessibilityRoot = accessibilityRoot;

			const accessibilityAnnouncement = document.createElement('div');
			accessibilityAnnouncement.setAttribute('aria-atomic', 'true');
			accessibilityAnnouncement.setAttribute('aria-live', 'polite');
			accessibilityAnnouncement.setAttribute('role', 'status');
			accessibilityAnnouncement.style.position = 'absolute';
			accessibilityAnnouncement.style.width = '1px';
			accessibilityAnnouncement.style.height = '1px';
			accessibilityAnnouncement.style.margin = '-1px';
			accessibilityAnnouncement.style.padding = '0';
			accessibilityAnnouncement.style.border = '0';
			accessibilityAnnouncement.style.overflow = 'hidden';
			accessibilityAnnouncement.style.clipPath = 'inset(50%)';
			accessibilityAnnouncement.style.whiteSpace = 'nowrap';
			root.appendChild(accessibilityAnnouncement);
			this.#accessibilityAnnouncement = accessibilityAnnouncement;

			const historyScrollbar = document.createElement('div');
			historyScrollbar.dataset['projectedTerminalHistoryScrollbar'] = 'true';
			historyScrollbar.setAttribute('aria-hidden', 'true');
			historyScrollbar.hidden = true;
			historyScrollbar.style.position = 'absolute';
			historyScrollbar.style.top = '0';
			historyScrollbar.style.right = '0';
			historyScrollbar.style.width = '16px';
			historyScrollbar.style.height = '100%';
			historyScrollbar.style.touchAction = 'none';
			historyScrollbar.style.zIndex = '1';
			const historyScrollbarThumb = document.createElement('div');
			historyScrollbarThumb.dataset['projectedTerminalHistoryScrollbarThumb'] = 'true';
			historyScrollbarThumb.style.position = 'absolute';
			historyScrollbarThumb.style.right = '2px';
			historyScrollbarThumb.style.width = '8px';
			historyScrollbarThumb.style.background = 'rgb(128, 128, 128)';
			historyScrollbarThumb.style.pointerEvents = 'none';
			historyScrollbarThumb.style.opacity = '0';
			historyScrollbar.appendChild(historyScrollbarThumb);
			root.appendChild(historyScrollbar);
			this.#historyScrollbar = historyScrollbar;
			this.#historyScrollbarThumb = historyScrollbarThumb;
			document.addEventListener('selectionchange', this.#handleAccessibilitySelectionChange);
			this.#listenerDisposers.push(() =>
				document.removeEventListener('selectionchange', this.#handleAccessibilitySelectionChange),
			);

			const renderer = new WebGPURenderer(canvas, {
				...this.#options.renderer,
				...presentationConfiguration,
				devicePixelRatio: this.#devicePixelRatio,
			});
			renderer.setHealthChangeCallback((event) => this.#healthEmitter.fire(event));
			renderer.setFrameTelemetryCallback((event) => {
				const metadata = this.#visibleStore().metadata;
				if (metadata === null) return;
				this.#frameTelemetryEmitter.fire({
					...event,
					generation: CanonicalUint64TextSchema.parse(metadata.generation.toString()),
					revision: CanonicalUint64TextSchema.parse(metadata.revision.toString()),
				});
			});
			renderer.setRenderRequestCallback(() => this.#scheduleRender());
			this.#renderer = renderer;
			this.#installInputListeners(root, canvas, input, historyScrollbar);
			await renderer.initGPU();
			if (this.#isDisposed()) return;
			this.#installDevicePixelRatioListeners(view);
			this.#installResizeObserver(root);
			this.#phase = ProjectedTerminalPhase.open;
			this.#updateSurfaceMouseCursor();
			this.#syncRendererSuspension();
			this.#sendResizeIfChanged();
			this.#sendDemandIfNeeded();
		} catch (error) {
			if (!this.#isDisposed()) {
				this.#rollbackOpen();
				this.#errorEmitter.fire({ error, operation: 'open' });
			}
			throw error;
		}
	}

	/** Replace every client-owned presentation field without reopening this attachment. */
	async updatePresentationConfiguration(
		candidate: unknown,
	): Promise<TerminalPresentationConfigurationUpdateResult> {
		const normalized = normalizeTerminalPresentationConfiguration(candidate);
		if (normalized.kind === 'invalid') {
			return {
				code: 'invalid-configuration',
				field: normalized.field,
				kind: 'failed',
			};
		}
		if (this.#isDisposed()) return { kind: 'cancelled', reason: 'disposed' };
		if (this.#phase !== ProjectedTerminalPhase.open) {
			return { code: 'terminal-not-open', kind: 'failed' };
		}
		const root = this.#root;
		const renderer = this.#renderer;
		const input = this.#input;
		if (!root || !renderer || !input) return { code: 'terminal-not-open', kind: 'failed' };

		this.#cancelPresentationPreparation('superseded');
		const epoch = ++this.#presentationRequestEpoch;
		const controller = new AbortController();
		this.#presentationPreparation = { controller, epoch };
		const fontPreparation = this.#loadConfiguredFonts(
			root.ownerDocument,
			normalized.configuration,
		).then(
			(): PresentationFontPreparation => ({ kind: 'ready' }),
			(error: unknown): PresentationFontPreparation => ({ error, kind: 'failed' }),
		);
		const outcome = await Promise.race([
			fontPreparation,
			this.#waitForPresentationCancellation(controller),
		]);
		if (this.#presentationPreparation?.epoch === epoch) {
			this.#presentationPreparation = undefined;
		}
		if (outcome.kind === 'cancelled') return outcome;
		if (this.#isDisposed()) return { kind: 'cancelled', reason: 'disposed' };
		if (epoch !== this.#presentationRequestEpoch) {
			return { kind: 'cancelled', reason: 'superseded' };
		}
		if (outcome.kind === 'failed') {
			this.#errorEmitter.fire({ error: outcome.error, operation: 'font' });
			return { code: 'font-preparation-failed', kind: 'failed' };
		}

		let prepared: PreparedTerminalPresentationConfiguration;
		try {
			prepared = renderer.preparePresentationConfiguration(normalized.configuration);
		} catch {
			return { code: 'renderer-preparation-failed', kind: 'failed' };
		}
		this.#commitPresentationConfiguration(renderer, input, prepared);
		return { configuration: prepared.configuration, kind: 'completed' };
	}

	applyFrame(frame: ArrayBuffer | Uint8Array): ProjectedFrameResult {
		if (this.#isDisposed()) return 'disposed';
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
				this.#reconcileCommittedGeometry(metadata);
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
					this.#metadataEmitter.fire(this.#metadataWithLiveCloseActivity(metadata));
					this.#updateAccessibility(metadata);
				}
				this.#syncPreeditPresentation();
				this.#updateSurfaceMouseCursor();
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
		if (this.#phase === ProjectedTerminalPhase.disposed) return;
		const presentationPaused = demand === 'paused';
		const demandChanged = demand !== this.#demand;
		this.#demand = demand;
		this.#presentationPaused = presentationPaused;
		if (demandChanged) this.#sendDemandIfNeeded();
		this.#syncRendererSuspension();
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
		if (
			window.generation !== this.#historyGeneration ||
			window.revision !== this.#historyRevision
		) {
			if (flight.kind === 'buffer') {
				if (!flight.aborted) {
					flight.resolve({ ...this.getBufferRowsSnapshot(), kind: 'stale' });
				}
			} else {
				this.#pendingHistoryTransition ??= 'awaiting-change';
			}
			this.#dispatchPendingHistoryIntent();
			return 'ignored';
		}
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
		const visibleStartRow = window.startRow;
		const visibleEnd = visibleStartRow + BigInt(flight.visibleRowCount);
		if (
			(flight.expectedStartRow !== null && visibleStartRow !== flight.expectedStartRow) ||
			window.frame.rows < flight.visibleRowCount ||
			visibleEnd > window.totalRows
		) {
			this.#errorEmitter.fire({
				error: new Error('Projected history window does not begin at the requested viewport'),
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
		const viewport = new PackedTerminalViewport(staged, 0, flight.visibleRowCount);
		this.#historyStore?.dispose();
		this.#historyStore = viewport;
		this.#invalidateLinkCache();
		this.#historyGeneration = window.generation;
		this.#historyRevision = window.revision;
		this.#historyStartRow = visibleStartRow;
		this.#historyTotalRows = window.totalRows;
		this.#historyRowsBefore = Number(
			window.totalRows - (visibleStartRow + BigInt(flight.visibleRowCount)),
		);
		this.#historyPhase = ProjectedHistoryPhase.viewingHistory;
		this.#syncPreeditPresentation();
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
			this.#metadataEmitter.fire(this.#metadataWithLiveCloseActivity(metadata));
			this.#updateAccessibility(metadata);
		}
		this.#continueSelectionAutoscrollGesture();
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
		this.#showHistoryScrollbar();
		this.#invalidateLinkCache();
		if (this.#pendingHistoryTransition !== undefined) {
			this.#pendingHistoryDelta = clamp(
				this.#pendingHistoryDelta + rows,
				-PROTOCOL_LIMITS.maxRowWindowRows,
				PROTOCOL_LIMITS.maxRowWindowRows,
			);
			return;
		}
		if (this.#rowFlight !== undefined) {
			this.#cancelBufferFlightForViewportIntent();
			this.#pendingHistoryStartRow = undefined;
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
		this.#showHistoryScrollbar();
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
				sequence: event.sequence,
			},
		});
		this.#historyEmitter.fire(event);
		if (changed) this.#searchController?.invalidate();
		if (changed) this.#invalidateLinkCache();
		if (
			this.#historyPhase === ProjectedHistoryPhase.followingBottom ||
			this.#historyPhase === ProjectedHistoryPhase.disposed
		) {
			return;
		}
		if (this.#rowFlight !== undefined && !changed) return;
		this.#pendingHistoryTransition = event.anchorStatus === 'valid' ? 'refresh' : 'recover';
		this.#dispatchPendingHistoryIntent();
	}

	notifySelectionChanged(selection: SelectionChanged): void {
		if (selection.attachmentId !== this.#attachmentId) return;
		const completedGesture = this.#selectionGestureActionInFlight;
		const suppressCopy = this.#suppressNextSelectionCopy;
		this.#selection = selection;
		if (!selectionMatchesSearch(selection, this.#searchOwnedSelection)) {
			this.#searchOwnedSelection = null;
		}
		this.#searchController?.setNavigationAnchor(searchAnchorFromSelection(selection));
		this.#selectionGestureActionInFlight = undefined;
		this.#selectionGestureInFlight = false;
		this.#suppressNextSelectionCopy = false;
		this.#selectionEmitter.fire(selection);
		this.#updateAccessibilityDescription();
		this.#syncAccessibilitySelection();
		this.#scheduleSelectionGesture();
		this.#scheduleRender();
		if (
			selection.state === 'active' &&
			this.#copyOnSelect &&
			this.#phase !== ProjectedTerminalPhase.disposed &&
			!suppressCopy &&
			completedGesture === 'end'
		) {
			this.#requestSelectionCopy('automatic');
		}
	}

	notifySelectionCopied(byteLength: number, requestSequence: CanonicalUint64Text): void {
		this.#completeSelectionCopy(requestSequence, { byteLength, kind: 'completed' });
		this.#selectionCopiedEmitter.fire({ byteLength, requestSequence });
	}

	notifyLinkOpened(requestSequence: CanonicalUint64Text): void {
		const complete = this.#linkCompletions.get(requestSequence);
		this.#linkCompletions.delete(requestSequence);
		complete?.({ kind: 'completed' });
	}

	notifyBrokerFailure(failure: ProjectedBrokerFailure): void {
		if (failure.operation === 'resize') this.#lastGeometry = undefined;
		let handledHistoryFailure = false;
		const repairedStaleSelectionGesture =
			failure.operation === 'selection' &&
			this.#selectionGestureActionInFlight !== undefined &&
			(failure.code === 'stale-generation' || failure.code === 'stale-revision');
		const activeFlight = this.#rowFlight;
		const flight =
			(failure.operation === undefined || failure.operation === 'history') &&
			failure.requestSequence !== undefined &&
			activeFlight?.requestSequence === failure.requestSequence
				? activeFlight
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
				handledHistoryFailure = true;
			} else if (
				this.#pendingHistoryTransition === 'recover' ||
				this.#pendingHistoryTransition === 'refresh'
			) {
				this.#dispatchPendingHistoryIntent();
				handledHistoryFailure = true;
			} else if (failure.code === 'stale-generation' || failure.code === 'stale-revision') {
				this.#pendingHistoryTransition = 'awaiting-change';
				handledHistoryFailure = true;
			} else if (failure.code === 'limit-exceeded' && flight.overscanned) {
				this.#requestHistoryRows(flight.origin, 0);
				handledHistoryFailure = true;
			} else {
				this.#recoverHistory();
			}
		}
		if (failure.requestSequence !== undefined && failure.operation === 'copy') {
			this.#completeSelectionCopy(failure.requestSequence, {
				code: failure.code,
				kind: 'failed',
				retryable: failure.retryable,
			});
		}
		if (failure.requestSequence !== undefined && failure.operation === 'link') {
			const complete = this.#linkCompletions.get(failure.requestSequence);
			this.#linkCompletions.delete(failure.requestSequence);
			complete?.({ code: failure.code, kind: 'failed', retryable: failure.retryable });
		}
		if (failure.operation === undefined || failure.operation === 'selection') {
			this.#selectionGestureActionInFlight = undefined;
			this.#selectionGestureInFlight = false;
			this.#selectionClickSequence = undefined;
			this.#suppressNextSelectionCopy = false;
			this.#scheduleSelectionGesture();
		}
		if (!handledHistoryFailure && !repairedStaleSelectionGesture) {
			this.#errorEmitter.fire({ ...failure, error: new Error(failure.message) });
		}
	}

	clearSelection(): void {
		this.#sendSelectionCommand('clear');
	}

	clear(): void {
		if (!this.canControlTerminal) return;
		this.#send({
			kind: 'clear-terminal',
			payload: { attachmentId: this.#attachmentId },
		});
	}

	reset(): void {
		if (!this.#beginInputRepair()) return;
		this.#stageActiveInputRepair(false, false);
		this.#clearHistoryScrollbarClickSuppression();
		this.#finishHistoryScrollbarGesture(true);
		if (!this.canControlTerminal) {
			this.#dispatchInputRepair();
			return;
		}
		this.#cancelSelectionGestureForReset();
		this.clearSelection();
		this.clearSearch();
		this.#invalidateLinkCache();
		if (this.#historyPhase !== ProjectedHistoryPhase.followingBottom) this.scrollToBottom();
		this.#dispatchInputRepair(() => {
			if (this.#inputAdmission !== ProjectedInputAdmission.repairing || !this.canControlTerminal) {
				return;
			}
			this.#send({
				kind: 'reset-terminal',
				payload: { attachmentId: this.#attachmentId },
			});
		});
	}

	selectAll(): void {
		this.#sendSelectionCommand('select-all');
	}

	setCopyOnSelect(enabled: boolean): void {
		if (typeof enabled === 'boolean') this.#copyOnSelect = enabled;
	}

	setResizeOwnership(ownsResizeLease: boolean): void {
		if (typeof ownsResizeLease !== 'boolean' || this.#ownsResizeLease === ownsResizeLease) return;
		this.#ownsResizeLease = ownsResizeLease;
		this.#lastGeometry = undefined;
		this.#lastGeometryAttemptedAt = Number.NEGATIVE_INFINITY;
		if (ownsResizeLease && this.#phase === ProjectedTerminalPhase.open) {
			this.#handleSurfaceResize();
		}
	}

	getSelectionRange(): ProjectedTerminalSelectionRange | null {
		const selection = this.#selection;
		if (selection.state !== 'active') return null;
		return {
			end: { column: selection.end.column, row: BigInt(selection.end.row) },
			rectangle: selection.rectangle,
			start: { column: selection.start.column, row: BigInt(selection.start.row) },
		};
	}

	setSelectionRange(
		start: ProjectedTerminalSelectionPoint,
		end: ProjectedTerminalSelectionPoint,
		rectangle = false,
	): boolean {
		if (typeof rectangle !== 'boolean') return false;
		const encodedStart = this.#selectionPointForCurrentBuffer(start);
		const encodedEnd = this.#selectionPointForCurrentBuffer(end);
		if (encodedStart === null || encodedEnd === null) return false;
		return this.#sendSelectionRange(encodedStart, encodedEnd, rectangle);
	}

	selectOutputAt(point: ProjectedTerminalSelectionPoint): boolean {
		const encodedPoint = this.#selectionPointForCurrentBuffer(point);
		if (encodedPoint === null) return false;
		this.#selectionClickSequence = undefined;
		return this.#send({
			kind: 'selection-command',
			payload: {
				attachmentId: this.#attachmentId,
				command: 'select-output',
				generation: CanonicalUint64TextSchema.parse(this.#historyGeneration.toString()),
				point: encodedPoint,
				revision: CanonicalUint64TextSchema.parse(this.#historyRevision.toString()),
			},
		});
	}

	adjustSelectionEndpoint(point: ProjectedTerminalSelectionPoint): boolean {
		const selection = this.#selection;
		if (selection.state !== 'active') return false;
		const encodedEnd = this.#selectionPointForCurrentBuffer(point);
		if (encodedEnd === null) return false;
		return this.#sendSelectionRange(selection.start, encodedEnd, selection.rectangle);
	}

	copySelection(): CanonicalUint64Text {
		return this.#requestSelectionCopy('manual');
	}

	copySelectionAsync(): Promise<ProjectedTerminalSideEffectResult> {
		return new Promise((resolve) => {
			this.#requestSelectionCopy('manual', resolve);
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
		this.#getSearchController().update(query, options, searchAnchorFromSelection(this.#selection));
	}

	findNext(): boolean {
		return this.#searchController?.next(false) ?? false;
	}

	findPrevious(): boolean {
		return this.#searchController?.next(true) ?? false;
	}

	clearSearch(): void {
		if (this.#searchController === undefined) {
			this.#renderer?.setSearchMatches([]);
			return;
		}
		this.#clearOwnedSearchSelection();
		this.#searchController.update('');
	}

	focus(): void {
		this.#input?.focus({ preventScroll: true });
	}

	async paste(text: string, source: ProjectedPasteSource = 'api'): Promise<ProjectedPasteResult> {
		const admitted = this.#canAdmitInput();
		return await this.#inputDispatcher.dispatch(
			(signal) => this.#performPaste(text, source, admitted, signal),
			() => this.#cancelledPasteResult(text, source),
		);
	}

	async #performPaste(
		text: string,
		source: ProjectedPasteSource,
		admitted: boolean,
		signal: AbortSignal,
	): Promise<ProjectedPasteResult> {
		const prepared = this.#preparePaste(text, source);
		if (prepared === undefined) return { kind: 'empty' };
		const { review, sanitized } = prepared;
		if (!admitted || signal.aborted || !this.#canAdmitInput()) {
			return { kind: 'blocked', review };
		}
		const protectedByBracketedPaste =
			review.bracketed && review.multiline && !review.embeddedBracketTerminator;
		const requiresReview =
			review.source === 'file-drop' ||
			review.replacedControlCharacters > 0 ||
			review.embeddedBracketTerminator ||
			(review.multiline && !protectedByBracketedPaste);
		if (requiresReview) {
			const approved = await this.#options.reviewPaste?.(review, signal);
			if (approved !== true) return { kind: 'blocked', review };
		}
		if (signal.aborted || !this.#canAdmitInput()) {
			return { kind: 'blocked', review };
		}
		const sent = this.#publishCurrentInputCommand(
			{
				kind: 'paste-input',
				payload: {
					attachmentId: this.#attachmentId,
					bracketed: review.bracketed,
					text: sanitized,
				},
			},
			ProjectedInputPublicationIntent.stateless,
		);
		return sent ? { kind: 'sent', review } : { kind: 'blocked', review };
	}

	#preparePaste(text: string, source: ProjectedPasteSource): PreparedPaste | undefined {
		if (!text) return undefined;
		const { sanitized, replaced } = sanitizePaste(text);
		if (!sanitized || textEncoder.encode(sanitized).byteLength > PROTOCOL_LIMITS.maxInputBytes) {
			return undefined;
		}
		const review: ProjectedPasteReview = {
			bracketed: this.#store.metadata?.interaction.bracketedPaste ?? false,
			embeddedBracketTerminator: text.includes(BRACKETED_PASTE_END),
			multiline: sanitized.includes('\n'),
			originalText: text,
			replacedControlCharacters: replaced,
			source,
			text: sanitized,
		};
		return { review, sanitized };
	}

	#cancelledPasteResult(text: string, source: ProjectedPasteSource): ProjectedPasteResult {
		const prepared = this.#preparePaste(text, source);
		return prepared === undefined
			? { kind: 'empty' }
			: { kind: 'blocked', review: prepared.review };
	}

	notifyBell(event: TerminalBell): void {
		if (this.#phase === ProjectedTerminalPhase.disposed) return;
		if (event.attachmentId !== this.#attachmentId) {
			this.#errorEmitter.fire({
				error: new Error('Terminal bell identity does not match the projected terminal'),
				operation: 'bell',
			});
			return;
		}
		const sequence = BigInt(event.sequence);
		if (sequence <= this.#latestBellSequence) return;
		this.#latestBellSequence = sequence;
		this.#bellEmitter.fire(event);
	}

	notifyClipboardWriteRequest(event: TerminalClipboardWriteRequest): void {
		if (this.#phase === ProjectedTerminalPhase.disposed) return;
		if (event.attachmentId !== this.#attachmentId) {
			this.#errorEmitter.fire({
				error: new Error(
					'Terminal clipboard request identity does not match the projected terminal',
				),
				operation: 'clipboard',
			});
			return;
		}
		const sequence = BigInt(event.sequence);
		if (sequence <= this.#latestClipboardWriteSequence) return;
		this.#latestClipboardWriteSequence = sequence;
		this.#clipboardWriteRequestEmitter.fire(event);
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
		this.#sealExitedControl();
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

	#sealExitedControl(): void {
		if (this.#terminalExited) return;
		this.#terminalExited = true;
		this.#inputEnabled = false;
		this.#ownsResizeLease = false;
		this.#lastGeometry = undefined;
		this.#lastGeometryAttemptedAt = Number.NEGATIVE_INFINITY;
		this.#input?.setAttribute('readonly', '');
		this.#inputDispatcher.invalidate();
		this.#inputRepairEpoch++;
		this.#repairTransientInputState();
		this.#deferredKeyReleases.clear();
		this.#pressedKeys.clear();
		this.#publishedKeys.clear();
		this.#pressedMouseButtons.clear();
		this.#publishedMouseButtons.clear();
		this.#publishedMouseSample = undefined;
		this.#publishedInputFocused = false;
		this.#finishApplicationMouseGesture(true);
		this.#pressedModifierCodes.clear();
		this.#resetSurfaceMouseModifiers();
		this.#clearApplicationMouseClickSuppression();
	}

	dispose(): void {
		if (this.#phase === ProjectedTerminalPhase.disposed) return;
		if (this.#phase === ProjectedTerminalPhase.frozen) {
			this.#performDispose(true, false);
			return;
		}
		if (this.#inputAdmission === ProjectedInputAdmission.disposed) return;
		if (this.#inputRepairTransaction !== undefined) {
			this.#inputAdmission = ProjectedInputAdmission.disposed;
			this.#inputRepairEpoch++;
			this.#inputDispatcher.invalidate();
			this.#disposeAfterRepair = true;
			this.#stageActiveInputRepair(true, true);
			return;
		}
		this.#inputAdmission = ProjectedInputAdmission.disposed;
		this.#inputRepairEpoch++;
		this.#inputDispatcher.invalidate();
		if (this.#inputPublicationActive) {
			this.#inputRepairTransaction = this.#createInputRepairTransaction();
			this.#disposeAfterRepair = true;
			this.#stageActiveInputRepair(true, true);
			this.#dispatchInputRepair();
			return;
		}
		this.#performDispose();
	}

	/**
	 * Release live attachment behavior while leaving the last painted canvas and accessible rows
	 * mounted for an application-owned recovery handoff.
	 */
	freeze(): void {
		if (
			this.#phase === ProjectedTerminalPhase.disposed ||
			this.#phase === ProjectedTerminalPhase.frozen ||
			this.#inputAdmission === ProjectedInputAdmission.disposed
		) {
			return;
		}
		this.#inputAdmission = ProjectedInputAdmission.disposed;
		this.#inputRepairEpoch++;
		this.#inputDispatcher.invalidate();
		this.#cancelPresentationPreparation('disposed');
		this.#prepareFocusLossInputRepair(true);
		this.#phase = ProjectedTerminalPhase.frozen;
		this.#historyPhase = ProjectedHistoryPhase.disposed;
		this.#cancelScheduledFrame();
		this.#cancelScheduledSelectionGesture();
		this.#clearApplicationMouseClickSuppression();
		this.#clearHistoryScrollbarClickSuppression();
		this.#cancelHistoryScrollbarFade();
		this.#stopSelectionAutoscroll();
		this.#searchController?.dispose();
		this.#searchController = undefined;
		this.#clearHoveredLink();
		for (const complete of this.#copyCompletions.values()) complete({ kind: 'cancelled' });
		for (const complete of this.#linkCompletions.values()) complete({ kind: 'cancelled' });
		this.#copyCompletions.clear();
		this.#copyRequests.clear();
		this.#linkCompletions.clear();
		const rowFlight = this.#rowFlight;
		this.#rowFlight = undefined;
		this.#pendingHistoryTransition = undefined;
		if (rowFlight?.kind === 'buffer') {
			rowFlight.removeAbortListener();
			if (!rowFlight.aborted) {
				rowFlight.resolve({ ...this.getBufferRowsSnapshot(), kind: 'cancelled' });
			}
		}
		this.#freezeSurfaceBindings();
		this.#releaseHistoryScrollbarDom();
		this.#input?.remove();
		this.#input = undefined;
		this.#accessibilityAnnouncement?.remove();
		this.#accessibilityAnnouncement = undefined;
	}

	#rollbackOpen(): void {
		const inputCleanup = this.#prepareFocusLossInputRepair(false);
		this.#dispatchDisposedInputCleanup(inputCleanup);
		this.#cancelScheduledFrame();
		this.#releaseSurfaceBindings();
		this.#releaseSurfaceDom();
		this.#documentHidden = false;
		this.#devicePixelRatio = 1;
		this.#lastGeometry = undefined;
		this.#lastGeometryAttemptedAt = Number.NEGATIVE_INFINITY;
		this.#presentationSuspended = false;
		this.#demandSent = undefined;
		this.#phase = ProjectedTerminalPhase.constructed;
	}

	#performDispose(inputCleanupAttempted = false, publishLifecycle = true): void {
		if (this.#disposalStarted) return;
		this.#disposalStarted = true;
		this.#cancelPresentationPreparation('disposed');
		const inputCleanup = inputCleanupAttempted ? [] : this.#prepareFocusLossInputRepair(true);
		if (publishLifecycle && this.#historyPhase !== ProjectedHistoryPhase.followingBottom) {
			this.#send({
				kind: 'set-history-demand',
				payload: { attachmentId: this.#attachmentId, demand: 'following-bottom' },
			});
		}
		if (publishLifecycle && this.#demandSent === 'active') {
			this.#demand = 'paused';
			this.#sendDemandIfNeeded();
		}
		this.#phase = ProjectedTerminalPhase.disposed;
		this.#historyPhase = ProjectedHistoryPhase.disposed;
		if (publishLifecycle) this.#dispatchDisposedInputCleanup(inputCleanup);
		const rowFlight = this.#rowFlight;
		this.#rowFlight = undefined;
		this.#pendingHistoryTransition = undefined;
		if (rowFlight?.kind === 'buffer') {
			rowFlight.removeAbortListener();
			if (!rowFlight.aborted) {
				rowFlight.resolve({ ...this.getBufferRowsSnapshot(), kind: 'cancelled' });
			}
		}
		this.#cancelScheduledFrame();
		this.#cancelScheduledSelectionGesture();
		this.#clearApplicationMouseClickSuppression();
		this.#clearHistoryScrollbarClickSuppression();
		this.#cancelHistoryScrollbarFade();
		this.#pendingHistoryDelta = 0;
		this.#pendingHistoryStartRow = undefined;
		this.#pendingHistoryTargetRow = undefined;
		this.#stopSelectionAutoscroll();
		this.#searchController?.dispose();
		this.#searchController = undefined;
		this.#clearHoveredLink();
		for (const complete of this.#copyCompletions.values()) complete({ kind: 'cancelled' });
		for (const complete of this.#linkCompletions.values()) complete({ kind: 'cancelled' });
		this.#copyCompletions.clear();
		this.#copyRequests.clear();
		this.#linkCompletions.clear();
		this.#releaseSurfaceBindings();
		this.#store.dispose();
		this.#historyStore?.dispose();
		this.#historyStore = undefined;
		this.#releaseSurfaceDom();
		this.#errorEmitter.dispose();
		this.#clipboardWriteRequestEmitter.dispose();
		this.#exitEmitter.dispose();
		this.#frameTelemetryEmitter.dispose();
		this.#healthEmitter.dispose();
		this.#historyEmitter.dispose();
		this.#linkOpenEmitter.dispose();
		this.#metadataEmitter.dispose();
		this.#selectionEmitter.dispose();
		this.#selectionCopyResultEmitter.dispose();
		this.#selectionCopiedEmitter.dispose();
		this.#searchEmitter.dispose();
	}

	#releaseSurfaceBindings(): void {
		this.#releaseSurfaceListeners();
		this.#renderer?.dispose();
		this.#renderer = undefined;
	}

	#freezeSurfaceBindings(): void {
		this.#releaseSurfaceListeners();
		this.#renderer?.freeze();
	}

	#releaseSurfaceListeners(): void {
		this.#resizeObserver?.disconnect();
		this.#resizeObserver = undefined;
		this.#devicePixelRatioQuery?.removeEventListener('change', this.#handleDevicePixelRatioChange);
		this.#devicePixelRatioQuery = undefined;
		for (const dispose of this.#listenerDisposers.splice(0)) dispose();
	}

	#releaseSurfaceDom(): void {
		this.#canvas?.remove();
		this.#releaseHistoryScrollbarDom();
		this.#input?.remove();
		this.#accessibilityRoot?.remove();
		this.#accessibilityAnnouncement?.remove();
		this.#accessibilityModel.update([]);
		this.#accessibilityModelRows.length = 0;
		this.#accessibilityRows.length = 0;
		this.#accessibilityAppliedOffsets = undefined;
		this.#accessibilityOwnsDomSelection = false;
		this.#accessibilityPreviousCursor = undefined;
		this.#accessibilityPreviousModelRows = [];
		this.#accessibilityPreviousRows = [];
		this.#canvas = undefined;
		this.#input = undefined;
		this.#accessibilityAnnouncement = undefined;
		this.#accessibilityRoot = undefined;
		this.#root = undefined;
	}

	#releaseHistoryScrollbarDom(): void {
		this.#historyScrollbar?.remove();
		this.#historyScrollbar = undefined;
		this.#historyScrollbarThumb = undefined;
	}

	#scheduleRender(): boolean {
		if (this.#isDisposed() || !this.#renderer) return false;
		if (this.#presentationSuspended) return true;
		this.#syncProjectedViewportState();
		this.#updateInputPosition();
		if (this.#frameHandle !== undefined) return true;
		const callback = (): void => {
			this.#frameHandle = undefined;
			if (this.#isDisposed()) return;
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

	#cancelScheduledFrame(): void {
		if (this.#frameHandle?.kind === 'animation-frame') {
			this.#root?.ownerDocument.defaultView?.cancelAnimationFrame(this.#frameHandle.value);
		} else if (this.#frameHandle?.kind === 'timeout') {
			globalThis.clearTimeout(this.#frameHandle.value);
		}
		this.#frameHandle = undefined;
	}

	#syncRendererSuspension(): void {
		const suspended = this.#documentHidden || this.#presentationPaused;
		if (suspended === this.#presentationSuspended) return;
		this.#presentationSuspended = suspended;
		this.#renderer?.setSuspended(suspended);
		if (suspended) {
			this.#cancelScheduledFrame();
			return;
		}
		this.#scheduleRender();
	}

	#visibleStore(): PackedTerminalStore | PackedTerminalViewport {
		return this.#historyPhase === ProjectedHistoryPhase.followingBottom ||
			this.#historyStore === undefined
			? this.#store
			: this.#historyStore;
	}

	#metadataWithLiveCloseActivity(metadata: ProjectedTerminalMetadata): ProjectedTerminalMetadata {
		const closeActivity = this.#store.metadata?.closeActivity;
		if (closeActivity === undefined || closeActivity === metadata.closeActivity) return metadata;
		return { ...metadata, closeActivity };
	}

	#syncProjectedViewportState(): void {
		const renderer = this.#renderer;
		const metadata = this.metadata;
		if (!renderer || !metadata) return;
		const absoluteTop = this.#historyVisibleStart(metadata);
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
		this.#syncHistoryScrollbarPresentation();
	}

	#historyVisibleStart(metadata: ProjectedTerminalMetadata): bigint {
		const visibleRows = BigInt(metadata.rows);
		const maximumStart =
			this.#historyTotalRows > visibleRows ? this.#historyTotalRows - visibleRows : 0n;
		if (
			this.#historyPhase === ProjectedHistoryPhase.followingBottom ||
			this.#historyStore === undefined
		) {
			return maximumStart;
		}
		return this.#historyStartRow < 0n ? 0n : minBigInt(this.#historyStartRow, maximumStart);
	}

	#historyScrollbarProjection(): HistoryScrollbarProjection | null {
		const root = this.#root;
		const metadata = this.metadata;
		if (!root || !metadata) return null;
		const rectangle = root.getBoundingClientRect();
		return projectHistoryScrollbar({
			opacity: this.#historyScrollbarOpacity,
			totalRows: this.#historyTotalRows,
			viewportHeight: rectangle.height,
			viewportWidth: rectangle.width,
			visibleRows: metadata.rows,
			visibleStartRow: this.#historyVisibleStart(metadata),
		});
	}

	#syncHistoryScrollbarPresentation(): void {
		const scrollbar = this.#historyScrollbar;
		const thumb = this.#historyScrollbarThumb;
		if (scrollbar === undefined || thumb === undefined) return;
		const projection = this.#historyScrollbarProjection();
		if (projection === null) {
			scrollbar.hidden = true;
			thumb.style.opacity = '0';
			return;
		}
		scrollbar.hidden = false;
		scrollbar.style.width = `${projection.hitArea.width}px`;
		thumb.style.height = `${projection.thumb.height}px`;
		thumb.style.opacity = String(projection.thumb.opacity);
		thumb.style.top = `${projection.thumb.y}px`;
		thumb.style.width = `${projection.thumb.width}px`;
	}

	#showHistoryScrollbar(): void {
		if (this.#phase === ProjectedTerminalPhase.disposed) return;
		const metadata = this.metadata ?? this.#store.metadata;
		if (metadata === null || this.#historyTotalRows <= BigInt(metadata.rows)) return;
		this.#cancelHistoryScrollbarFade();
		this.#historyScrollbarOpacity = 1;
		this.#scheduleRender();
		if (this.#historyScrollbarGesture !== undefined) return;
		this.#historyScrollbarFadeTimer = globalThis.setTimeout(() => {
			this.#historyScrollbarFadeTimer = undefined;
			if (this.#phase === ProjectedTerminalPhase.disposed) return;
			this.#historyScrollbarOpacity = 0;
			this.#scheduleRender();
		}, HISTORY_SCROLLBAR_FADE_DELAY_MS);
	}

	#cancelHistoryScrollbarFade(): void {
		if (this.#historyScrollbarFadeTimer === undefined) return;
		globalThis.clearTimeout(this.#historyScrollbarFadeTimer);
		this.#historyScrollbarFadeTimer = undefined;
	}

	#armApplicationMouseClickSuppression(event: PointerEvent): void {
		this.#clearApplicationMouseClickSuppression();
		this.#applicationMouseClickSuppression = {
			clientX: event.clientX,
			clientY: event.clientY,
		};
		this.#applicationMouseClickSuppressionTimer = globalThis.setTimeout(() => {
			this.#applicationMouseClickSuppression = undefined;
			this.#applicationMouseClickSuppressionTimer = undefined;
		}, 0);
	}

	#clearApplicationMouseClickSuppression(): void {
		this.#applicationMouseClickSuppression = undefined;
		if (this.#applicationMouseClickSuppressionTimer === undefined) return;
		globalThis.clearTimeout(this.#applicationMouseClickSuppressionTimer);
		this.#applicationMouseClickSuppressionTimer = undefined;
	}

	#consumeApplicationMouseClickSuppression(event: MouseEvent): boolean {
		const suppression = this.#applicationMouseClickSuppression;
		this.#clearApplicationMouseClickSuppression();
		return (
			suppression !== undefined &&
			event.button === 0 &&
			event.clientX === suppression.clientX &&
			event.clientY === suppression.clientY
		);
	}

	#armHistoryScrollbarClickSuppression(event: PointerEvent): void {
		this.#clearHistoryScrollbarClickSuppression();
		this.#historyScrollbarClickSuppression = {
			clientX: event.clientX,
			clientY: event.clientY,
		};
		this.#historyScrollbarClickSuppressionTimer = globalThis.setTimeout(() => {
			this.#historyScrollbarClickSuppression = undefined;
			this.#historyScrollbarClickSuppressionTimer = undefined;
		}, 0);
	}

	#clearHistoryScrollbarClickSuppression(): void {
		this.#historyScrollbarClickSuppression = undefined;
		if (this.#historyScrollbarClickSuppressionTimer === undefined) return;
		globalThis.clearTimeout(this.#historyScrollbarClickSuppressionTimer);
		this.#historyScrollbarClickSuppressionTimer = undefined;
	}

	#consumeHistoryScrollbarClickSuppression(event: MouseEvent): boolean {
		const suppression = this.#historyScrollbarClickSuppression;
		this.#clearHistoryScrollbarClickSuppression();
		return (
			suppression !== undefined &&
			event.button === 0 &&
			event.clientX === suppression.clientX &&
			event.clientY === suppression.clientY
		);
	}

	#requestHistoryRows(origin: ReadRows['origin'], overscanRows = HISTORY_OVERSCAN_ROWS): void {
		const metadata = this.#store.metadata ?? this.#historyStore?.metadata;
		if (
			metadata === null ||
			metadata === undefined ||
			this.#rowFlight !== undefined ||
			this.#pendingHistoryTransition !== undefined ||
			this.#phase === ProjectedTerminalPhase.disposed
		) {
			return;
		}
		const visibleRowCount = metadata.rows;
		const visibleStartRow = this.#resolveHistoryVisibleStart(origin, visibleRowCount);
		const requestOrigin: ReadRows['origin'] =
			origin.kind === 'committed'
				? origin
				: {
						kind: 'absolute',
						row: CanonicalUint64TextSchema.parse(visibleStartRow.toString()),
					};
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
			Number(this.#historyTotalRows - visibleStartRow),
		);
		if (cacheRowCount < visibleRowCount) return;
		if (this.#historyPhase === ProjectedHistoryPhase.followingBottom) {
			this.#send({
				kind: 'set-history-demand',
				payload: { attachmentId: this.#attachmentId, demand: 'viewing-history' },
			});
			this.#demand = 'paused';
			this.#sendDemandIfNeeded();
		}
		this.#historyPhase = ProjectedHistoryPhase.requestingHistory;
		this.#syncPreeditPresentation();
		const requestSequence = this.#nextRowRequestSequence();
		this.#rowFlight = {
			expectedStartRow: origin.kind === 'committed' ? null : visibleStartRow,
			kind: 'viewport',
			maximumBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
			origin: requestOrigin,
			overscanned: cacheRowCount > visibleRowCount,
			requestSequence,
			visibleRowCount,
		};
		this.#send({
			kind: 'read-rows',
			payload: {
				attachmentId: this.#attachmentId,
				detail: 'render',
				generation: CanonicalUint64TextSchema.parse(this.#historyGeneration.toString()),
				maximumBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
				origin: requestOrigin,
				purpose: 'viewport',
				requestSequence,
				revision: CanonicalUint64TextSchema.parse(this.#historyRevision.toString()),
				rowCount: cacheRowCount,
			},
		});
	}

	#requestHistoryStartRow(requestedStartRow: bigint): void {
		const metadata = this.metadata ?? this.#store.metadata;
		if (metadata === null || this.#phase === ProjectedTerminalPhase.disposed) return;
		const visibleRows = BigInt(metadata.rows);
		const maximumStart =
			this.#historyTotalRows > visibleRows ? this.#historyTotalRows - visibleRows : 0n;
		const startRow = requestedStartRow < 0n ? 0n : minBigInt(requestedStartRow, maximumStart);
		this.#pendingHistoryDelta = 0;
		this.#pendingHistoryTargetRow = undefined;
		if (this.#rowFlight !== undefined || this.#pendingHistoryTransition !== undefined) {
			this.#cancelBufferFlightForViewportIntent();
			this.#pendingHistoryStartRow = startRow;
			return;
		}
		if (this.#historyPhase === ProjectedHistoryPhase.recovering) {
			this.#pendingHistoryStartRow = startRow;
			return;
		}
		this.#pendingHistoryStartRow = undefined;
		if (startRow === maximumStart) {
			this.#recoverHistory();
			return;
		}
		if (
			this.#historyPhase === ProjectedHistoryPhase.viewingHistory &&
			startRow === this.#historyStartRow
		) {
			return;
		}
		const rowsBefore = this.#historyTotalRows - (startRow + visibleRows);
		this.#historyRowsBefore = Number(
			rowsBefore > BigInt(Number.MAX_SAFE_INTEGER) ? BigInt(Number.MAX_SAFE_INTEGER) : rowsBefore,
		);
		this.#requestHistoryRows({
			kind: 'absolute',
			row: CanonicalUint64TextSchema.parse(startRow.toString()),
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
		const transition = this.#pendingHistoryTransition;
		if (transition === 'awaiting-change') return;
		this.#pendingHistoryTransition = undefined;
		if (transition === 'recover') {
			this.#recoverHistory();
			return;
		}
		if (transition === 'refresh') {
			this.#requestHistoryRows({ delta: 0, kind: 'committed' });
			return;
		}
		const startRow = this.#pendingHistoryStartRow;
		this.#pendingHistoryStartRow = undefined;
		if (startRow !== undefined) {
			this.#requestHistoryStartRow(startRow);
			return;
		}
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
		this.#pendingHistoryTransition = undefined;
		this.#pendingHistoryStartRow = undefined;
		this.#pendingHistoryTargetRow = undefined;
		this.#searchController?.invalidate();
		this.#historyPhase = ProjectedHistoryPhase.recovering;
		this.#syncPreeditPresentation();
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
		this.#syncPreeditPresentation();
		const metadata = this.#store.metadata;
		if (metadata) {
			this.#metadataEmitter.fire(this.#metadataWithLiveCloseActivity(metadata));
			this.#updateAccessibility(metadata);
		}
		this.#continueSelectionAutoscrollGesture();
		this.#scheduleRender();
		this.#dispatchPendingHistoryIntent();
	}

	#getSearchController(): ProjectedSearchController {
		this.#searchController ??= new ProjectedSearchController({
			createWorker: this.#options.searchWorkerFactory ?? createProjectedSearchWorker,
			onChange: (event) => {
				if (event.state !== 'searching' && event.activeMatch === null) {
					this.#clearOwnedSearchSelection();
				}
				this.#renderer?.setSearchMatches(event.matches, event.activeMatch);
				this.#searchEmitter.fire(event);
			},
			onReveal: (match, source) => this.#activateSearchMatch(match, source),
			reader: this,
		});
		return this.#searchController;
	}

	#activateSearchMatch(
		match: TerminalSearchMatch,
		source: { readonly generation: bigint; readonly revision: bigint },
	): void {
		if (
			source.generation !== this.#historyGeneration ||
			source.revision !== this.#historyRevision ||
			!Number.isSafeInteger(match.start.row) ||
			match.start.row < 0 ||
			!Number.isSafeInteger(match.end.row) ||
			match.end.row < 0
		) {
			return;
		}
		const selected = this.setSelectionRange(
			{ column: match.start.column, row: BigInt(match.start.row) },
			{ column: match.end.column, row: BigInt(match.end.row) },
		);
		if (!selected) return;
		this.#searchOwnedSelection = match;
		this.#revealSearchRow(BigInt(match.start.row));
	}

	#clearOwnedSearchSelection(): void {
		if (this.#searchOwnedSelection === null) return;
		this.#searchOwnedSelection = null;
		this.clearSelection();
	}

	#revealSearchRow(row: bigint): void {
		const metadata = this.metadata ?? this.#store.metadata;
		if (metadata === null || row < 0n || row >= this.#historyTotalRows) return;
		const visibleRows = BigInt(metadata.rows);
		const visibleStart =
			this.#historyPhase === ProjectedHistoryPhase.followingBottom
				? this.#historyTotalRows - visibleRows
				: this.#historyStartRow;
		this.#accessibilityVisibleStart = visibleStart;
		if (row >= visibleStart && row < visibleStart + visibleRows) {
			this.#scheduleRender();
			return;
		}
		this.#showHistoryScrollbar();
		if (this.#rowFlight !== undefined || this.#pendingHistoryTransition !== undefined) {
			this.#cancelBufferFlightForViewportIntent();
			this.#pendingHistoryStartRow = undefined;
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

	#linkPointIsCurrent(point: PendingLinkPoint): boolean {
		return (
			point.generation === this.#historyGeneration &&
			point.revision === this.#historyRevision &&
			point.linkEpoch === this.#linkEpoch
		);
	}

	#queueLinkLookup(point: PendingLinkPoint): void {
		if (!this.#linkPointIsCurrent(point)) {
			this.#clearHoveredLink();
			return;
		}
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
				if (!this.#linkPointIsCurrent(requestedPoint)) {
					this.#clearHoveredLink();
					continue;
				}
				const snapshot = this.getBufferRowsSnapshot();
				if (
					requestedPoint.generation !== snapshot.generation ||
					requestedPoint.revision !== snapshot.revision
				) {
					this.#clearHoveredLink();
					continue;
				}
				if (requestedPoint.row >= snapshot.totalRows) {
					this.#presentHoveredLink(undefined, requestedPoint);
					continue;
				}
				const abortController = new AbortController();
				this.#linkAbortController = abortController;
				const lookup = await this.#readCompleteLinkLogicalLine(
					snapshot,
					requestedPoint.row,
					abortController,
				);
				if (this.#linkAbortController === abortController) {
					this.#linkAbortController = undefined;
				}
				if (abortController.signal.aborted) continue;
				const currentPoint = this.#pendingLinkPoint ?? requestedPoint;
				this.#pendingLinkPoint = undefined;
				if (!this.#linkPointIsCurrent(currentPoint)) {
					this.#clearHoveredLink();
					continue;
				}
				if (lookup === undefined) {
					this.#presentHoveredLink(undefined, currentPoint);
					continue;
				}
				if (
					currentPoint.generation !== lookup.generation ||
					currentPoint.revision !== lookup.revision
				) {
					this.#pendingLinkPoint = currentPoint;
					continue;
				}
				const logicalStartRow = lookup.rows[0]?.index;
				const endRow = lookup.rows.at(-1)?.index;
				if (logicalStartRow === undefined || endRow === undefined) {
					this.#presentHoveredLink(undefined, currentPoint);
					continue;
				}
				const links = detectTerminalRowLinks(lookup.rows, this.metadata?.currentDirectory ?? '');
				this.#linkCache = {
					endRow,
					generation: lookup.generation,
					links,
					revision: lookup.revision,
					startRow: logicalStartRow,
				};
				if (currentPoint.row < logicalStartRow || currentPoint.row > endRow) {
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

	async #readCompleteLinkLogicalLine(
		snapshot: TerminalBufferRowsSnapshot,
		targetRow: bigint,
		abortController: AbortController,
	): Promise<ProjectedLinkLookup | undefined> {
		const startRow = targetRow > 0n ? targetRow - 1n : 0n;
		const rowCount = Math.min(3, Number(snapshot.totalRows - startRow));
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
		if (result.kind !== 'rows') return undefined;
		const rows = await this.#completeLinkLogicalLine(result, targetRow, abortController);
		return rows === undefined
			? undefined
			: { generation: result.generation, revision: result.revision, rows };
	}

	async #completeLinkLogicalLine(
		initial: TerminalBufferRows,
		targetRow: bigint,
		abortController: AbortController,
	): Promise<readonly TerminalBufferRow[] | undefined> {
		const window: ProjectedLinkWindow = {
			direction: undefined,
			pageRows: LINK_LOOKUP_INITIAL_PAGE_ROWS,
			remainingBytes: PROTOCOL_LIMITS.maxRowWindowBytes - initial.byteLength,
			rows: [...initial.rows],
		};
		const columns = Math.max(1, this.metadata?.columns ?? 1);
		const rowLimit = Math.max(
			1,
			Math.min(
				PROTOCOL_LIMITS.maxRowWindowRows,
				Math.floor(PROTOCOL_LIMITS.maxFrameCells / columns),
			),
		);
		return this.#extendLinkLogicalLine(window, initial, targetRow, rowLimit, abortController);
	}

	async #extendLinkLogicalLine(
		window: ProjectedLinkWindow,
		initial: TerminalBufferRows,
		targetRow: bigint,
		rowLimit: number,
		abortController: AbortController,
	): Promise<readonly TerminalBufferRow[] | undefined> {
		const logicalLine = logicalLineInWindow(window.rows, targetRow);
		if (logicalLine === undefined) return undefined;
		if (logicalLine.completeBefore && logicalLine.completeAfter) return logicalLine.rows;
		if (
			window.rows.length >= rowLimit ||
			window.remainingBytes < TERMINAL_ROW_WINDOW_HEADER_BYTES
		) {
			return undefined;
		}
		const direction = logicalLine.completeBefore ? 'forward' : 'backward';
		const pageRows =
			window.direction === direction ? window.pageRows : LINK_LOOKUP_INITIAL_PAGE_ROWS;
		const availableRows = Math.min(pageRows, rowLimit - window.rows.length);
		const boundaryRow =
			direction === 'backward' ? window.rows[0]?.index : window.rows.at(-1)?.index;
		if (boundaryRow === undefined) return undefined;
		const remainingRows =
			direction === 'backward' ? boundaryRow : initial.totalRows - (boundaryRow + 1n);
		const requestedRows = Number(
			remainingRows < BigInt(availableRows) ? remainingRows : BigInt(availableRows),
		);
		if (requestedRows <= 0) return undefined;
		const page = await this.#readLinkPage({
			abortController,
			direction,
			generation: initial.generation,
			maximumBytes: window.remainingBytes,
			revision: initial.revision,
			row: direction === 'backward' ? boundaryRow : boundaryRow + 1n,
			rowCount: requestedRows,
		});
		if (page === undefined) return undefined;
		window.remainingBytes -= page.byteLength;
		if (direction === 'backward') window.rows.unshift(...page.rows);
		else window.rows.push(...page.rows);
		window.direction = direction;
		window.pageRows = Math.min(pageRows * 2, rowLimit - window.rows.length);
		return this.#extendLinkLogicalLine(window, initial, targetRow, rowLimit, abortController);
	}

	async #readLinkPage(options: {
		readonly abortController: AbortController;
		readonly direction: 'backward' | 'forward';
		readonly generation: bigint;
		readonly maximumBytes: number;
		readonly revision: bigint;
		readonly row: bigint;
		readonly rowCount: number;
	}): Promise<TerminalBufferRows | undefined> {
		if (options.rowCount <= 0 || options.abortController.signal.aborted) return undefined;
		const startRow =
			options.direction === 'backward' ? options.row - BigInt(options.rowCount) : options.row;
		const result = await this.readBufferRows(
			{
				detail: 'render',
				generation: options.generation,
				maximumBytes: options.maximumBytes,
				revision: options.revision,
				rowCount: options.rowCount,
				startRow,
			},
			options.abortController.signal,
		);
		if (result.kind === 'too-large' && options.rowCount > 1) {
			return this.#readLinkPage({
				...options,
				rowCount: Math.max(1, Math.floor(options.rowCount / 2)),
			});
		}
		return result.kind === 'rows' ? result : undefined;
	}

	#surfaceApplicationMouseOwned(): boolean {
		if (this.#applicationMouseGesture !== undefined) return true;
		return (
			!this.#selectionGestureActive &&
			!this.#surfaceMouseModifiers.shift &&
			this.#applicationMouseTrackingOwnsSurface()
		);
	}

	#surfaceSelectionOwned(): boolean {
		if (this.#selectionGestureActive) return true;
		if (this.#applicationMouseGesture !== undefined) return false;
		return this.#surfaceMouseModifiers.shift || !this.#applicationMouseTrackingOwnsSurface();
	}

	#hoveredLinkIsActive(): boolean {
		const link = this.#hoveredLink;
		const point = this.#hoveredLinkPoint;
		return (
			this.#surfaceSelectionOwned() &&
			link !== undefined &&
			point !== undefined &&
			this.#linkPointIsCurrent(point) &&
			(this.#surfaceMouseModifiers.control || this.#surfaceMouseModifiers.meta) &&
			isPermittedExternalTerminalTarget(link)
		);
	}

	#setSurfaceMouseModifiers(
		event: Pick<MouseEvent, 'altKey' | 'ctrlKey' | 'metaKey' | 'shiftKey'>,
	): void {
		this.#surfaceMouseModifiers = {
			alt: event.altKey,
			control: event.ctrlKey,
			meta: event.metaKey,
			shift: event.shiftKey,
		};
		this.#updateSurfaceMouseCursor();
	}

	#resetSurfaceMouseModifiers(): void {
		this.#surfaceMouseModifiers = {
			alt: false,
			control: false,
			meta: false,
			shift: false,
		};
		this.#updateSurfaceMouseCursor();
	}

	#updateSurfaceMouseCursor(): void {
		const canvas = this.#canvas;
		if (canvas === undefined) return;
		canvas.style.cursor = resolveSurfaceMouseCursor({
			applicationMouseOwned: this.#surfaceApplicationMouseOwned(),
			linkActive: this.#hoveredLinkIsActive(),
			modifiers: this.#surfaceMouseModifiers,
			platform: this.#surfaceMousePlatform,
			selectionOwned: this.#surfaceSelectionOwned(),
		});
	}

	#presentHoveredLink(link: ILink | undefined, point: PendingLinkPoint): void {
		this.#hoveredLink = link;
		this.#hoveredLinkPoint = point;
		const enabled = this.#hoveredLinkIsActive();
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
		this.#updateSurfaceMouseCursor();
	}

	#clearHoveredLink(): void {
		this.#pendingLinkPoint = undefined;
		this.#linkAbortController?.abort();
		this.#linkAbortController = undefined;
		this.#hoveredLink = undefined;
		this.#hoveredLinkPoint = undefined;
		this.#renderer?.setHoveredLinkRange(null);
		this.#updateSurfaceMouseCursor();
	}

	#invalidateLinkCache(): void {
		this.#linkEpoch += 1n;
		this.#linkCache = undefined;
		this.#clearHoveredLink();
	}

	#sendSelectionCommand(command: 'clear' | 'select-all'): void {
		this.#selectionClickSequence = undefined;
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

	#selectionPointForCurrentBuffer(point: unknown): SelectionPoint | null {
		if (
			this.#phase === ProjectedTerminalPhase.disposed ||
			this.#historyGeneration === 0n ||
			typeof point !== 'object' ||
			point === null ||
			!('column' in point) ||
			!('row' in point) ||
			typeof point.row !== 'bigint'
		) {
			return null;
		}
		const parsed = SelectionPointSchema.safeParse({
			column: point.column,
			row: point.row.toString(),
		});
		const metadata = this.metadata;
		if (
			!parsed.success ||
			metadata === null ||
			parsed.data.column >= metadata.columns ||
			BigInt(parsed.data.row) >= this.#historyTotalRows
		) {
			return null;
		}
		return parsed.data;
	}

	#sendSelectionRange(start: SelectionPoint, end: SelectionPoint, rectangle = false): boolean {
		this.#selectionClickSequence = undefined;
		return this.#send({
			kind: 'selection-command',
			payload: {
				attachmentId: this.#attachmentId,
				command: 'set-range',
				end,
				generation: CanonicalUint64TextSchema.parse(this.#historyGeneration.toString()),
				rectangle,
				revision: CanonicalUint64TextSchema.parse(this.#historyRevision.toString()),
				start,
			},
		});
	}

	#queueSelectionGesture(gesture: PendingSelectionGesture): void {
		const lastIndex = this.#pendingSelectionGestures.length - 1;
		const last = this.#pendingSelectionGestures[lastIndex];
		if (
			last !== undefined &&
			last.action === 'update' &&
			(gesture.action === 'update' || gesture.action === 'cancel')
		) {
			this.#pendingSelectionGestures[lastIndex] = gesture;
		} else {
			const reservedEndpoint = gesture.action === 'start' ? 1 : 0;
			if (
				this.#pendingSelectionGestures.length >=
				MAX_PENDING_SELECTION_GESTURES - reservedEndpoint
			) {
				return;
			}
			this.#pendingSelectionGestures.push(gesture);
		}
		this.#scheduleSelectionGesture();
	}

	#canStartSelectionGesture(): boolean {
		if (
			!this.#selectionGestureInFlight &&
			this.#selectionGestureHandle === undefined &&
			this.#pendingSelectionGestures.length === 0
		) {
			return true;
		}
		return this.#pendingSelectionGestures.length < MAX_PENDING_SELECTION_GESTURES - 1;
	}

	#startSelectionGesture(gesture: PendingSelectionGesture): void {
		if (
			this.#selectionGestureInFlight ||
			this.#selectionGestureHandle !== undefined ||
			this.#pendingSelectionGestures.length > 0
		) {
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
			this.#pendingSelectionGestures.length === 0 ||
			this.#phase === ProjectedTerminalPhase.disposed
		) {
			return;
		}
		const callback = (): void => {
			this.#selectionGestureHandle = undefined;
			if (this.#selectionGestureInFlight) return;
			const gesture = this.#pendingSelectionGestures.shift();
			if (gesture === undefined || this.#phase === ProjectedTerminalPhase.disposed) return;
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
		this.#selectionGestureActionInFlight = gesture.action;
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

	#selectionPoint(event: MouseEvent): SelectionPoint | null {
		return this.#selectionPointFromClient(event.clientX, event.clientY);
	}

	#selectionPointFromClient(clientX: number, clientY: number): SelectionPoint | null {
		return this.#selectionGestureLocationFromClient(clientX, clientY)?.point ?? null;
	}

	#selectionGestureLocation(event: MouseEvent): SelectionGestureLocation | null {
		return this.#selectionGestureLocationFromClient(event.clientX, event.clientY);
	}

	#selectionGestureLocationFromClient(
		clientX: number,
		clientY: number,
	): SelectionGestureLocation | null {
		const canvas = this.#canvas;
		const renderer = this.#renderer;
		const metadata = this.metadata;
		if (!canvas || !renderer || !metadata) return null;
		const bounds = canvas.getBoundingClientRect();
		const surfaceX = clientX - bounds.left;
		const surfaceY = clientY - bounds.top;
		const column = clamp(Math.floor(surfaceX / renderer.charWidth), 0, metadata.columns - 1);
		const viewportRow = clamp(Math.floor(surfaceY / renderer.charHeight), 0, metadata.rows - 1);
		const startRow =
			this.#historyPhase === ProjectedHistoryPhase.followingBottom
				? this.#historyTotalRows - BigInt(metadata.rows)
				: this.#historyStartRow;
		return {
			geometry: {
				cellWidth: clamp(Math.round(renderer.charWidth), 1, PROTOCOL_LIMITS.maxCellPixelDimension),
				columns: metadata.columns,
				paddingLeft: 0,
				screenHeight: clamp(Math.round(bounds.height), 1, MAX_PTY_PIXEL_DIMENSION),
			},
			point: {
				column,
				row: CanonicalUint64TextSchema.parse((startRow + BigInt(viewportRow)).toString()),
			},
			position: {
				x: clamp(Math.trunc(surfaceX), -MAX_PTY_PIXEL_DIMENSION, MAX_PTY_PIXEL_DIMENSION),
				y: clamp(Math.trunc(surfaceY), -MAX_PTY_PIXEL_DIMENSION, MAX_PTY_PIXEL_DIMENSION),
			},
			surfaceX,
			surfaceY,
		};
	}

	#nextSelectionClickCount(event: PointerEvent, location: SelectionGestureLocation): number {
		const eventTime = event.timeStamp;
		const timeStamp = Number.isFinite(eventTime) && eventTime >= 0 ? eventTime : projectedNow();
		const previous = this.#selectionClickSequence;
		const repeatDistance = this.#renderer?.charWidth ?? location.geometry.cellWidth;
		const repeats =
			previous !== undefined &&
			previous.generation === this.#historyGeneration &&
			timeStamp >= previous.timeStamp &&
			timeStamp - previous.timeStamp <= SELECTION_CLICK_REPEAT_INTERVAL_MS &&
			Math.hypot(location.surfaceX - previous.surfaceX, location.surfaceY - previous.surfaceY) <=
				repeatDistance;
		const clickCount = repeats ? Math.min(previous.clickCount + 1, 3) : 1;
		this.#selectionClickSequence = {
			clickCount,
			generation: this.#historyGeneration,
			surfaceX: repeats ? previous.surfaceX : location.surfaceX,
			surfaceY: repeats ? previous.surfaceY : location.surfaceY,
			timeStamp,
		};
		return clickCount;
	}

	#updateSelectionAutoscroll(pointer: PendingSelectionPointer): void {
		this.#selectionPointer = pointer;
		const direction = this.#selectionAutoscrollDirectionAt(pointer.clientY);
		if (direction === this.#selectionAutoscrollDirection) return;
		this.#clearSelectionAutoscrollInterval();
		this.#selectionAutoscrollDirection = direction;
		if (direction === undefined) return;
		this.#selectionAutoscrollInterval = globalThis.setInterval(
			this.#runSelectionAutoscrollTick,
			SELECTION_AUTOSCROLL_INTERVAL_MS,
		);
	}

	#selectionAutoscrollDirectionAt(clientY: number): 'down' | 'up' | undefined {
		const bounds = this.#canvas?.getBoundingClientRect();
		if (bounds === undefined) return undefined;
		if (clientY < bounds.top) return 'up';
		return clientY > bounds.bottom ? 'down' : undefined;
	}

	readonly #runSelectionAutoscrollTick = (): void => {
		const pointer = this.#selectionPointer;
		const direction = this.#selectionAutoscrollDirection;
		if (
			!this.#selectionGestureActive ||
			pointer === undefined ||
			direction === undefined ||
			this.#selectionAutoscrollDirectionAt(pointer.clientY) !== direction
		) {
			this.#stopSelectionAutoscroll();
			return;
		}
		if (
			this.#rowFlight !== undefined ||
			this.#selectionGestureInFlight ||
			this.#pendingSelectionGestures.length > 0
		) {
			return;
		}
		this.scrollLines(direction === 'up' ? -1 : 1);
	};

	#continueSelectionAutoscrollGesture(): void {
		const pointer = this.#selectionPointer;
		if (!this.#selectionGestureActive || pointer === undefined) return;
		const location = this.#selectionGestureLocationFromClient(pointer.clientX, pointer.clientY);
		if (location === null) return;
		this.#selectionGestureLast = location.point;
		this.#queueSelectionGesture({
			action: 'update',
			geometry: location.geometry,
			point: location.point,
			position: location.position,
			rectangle: pointer.rectangle,
		});
	}

	#stopSelectionAutoscroll(): void {
		this.#clearSelectionAutoscrollInterval();
		this.#selectionAutoscrollDirection = undefined;
		this.#selectionPointer = undefined;
	}

	#clearSelectionAutoscrollInterval(): void {
		if (this.#selectionAutoscrollInterval === undefined) return;
		globalThis.clearInterval(this.#selectionAutoscrollInterval);
		this.#selectionAutoscrollInterval = undefined;
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
		return (
			this.#phase === ProjectedTerminalPhase.disposed ||
			this.#phase === ProjectedTerminalPhase.frozen
		);
	}

	#requestSelectionCopy(
		source: ProjectedTerminalSelectionCopyEvent['source'],
		complete?: (result: ProjectedTerminalSideEffectResult) => void,
	): CanonicalUint64Text {
		const requestSequence = this.#nextSideEffectSequence();
		if (this.#copyRequests.size > 0) {
			const result = { code: 'copy-in-progress', kind: 'failed', retryable: true } as const;
			complete?.(result);
			this.#selectionCopyResultEmitter.fire({ requestSequence, result, source });
			return requestSequence;
		}
		this.#copyRequests.set(requestSequence, source);
		if (complete !== undefined) this.#copyCompletions.set(requestSequence, complete);
		this.#send(
			{
				kind: 'selection-copy',
				payload: { attachmentId: this.#attachmentId, requestSequence },
			},
			() => {
				this.#completeSelectionCopy(requestSequence, {
					code: 'copy-send-failed',
					kind: 'failed',
					retryable: true,
				});
			},
		);
		return requestSequence;
	}

	#completeSelectionCopy(
		requestSequence: CanonicalUint64Text,
		result: ProjectedTerminalSelectionCopyResult,
	): void {
		const source = this.#copyRequests.get(requestSequence);
		if (source === undefined) return;
		this.#copyRequests.delete(requestSequence);
		const complete = this.#copyCompletions.get(requestSequence);
		this.#copyCompletions.delete(requestSequence);
		complete?.(result);
		this.#selectionCopyResultEmitter.fire({ requestSequence, result, source });
	}

	#send(command: ProjectedTerminalCommand, onFailure?: (error: unknown) => void): boolean {
		if (this.#phase === ProjectedTerminalPhase.frozen) return false;
		try {
			const pending = this.#options.port.send(command);
			void pending?.catch((error) => {
				this.#reportCommandFailure(command, error);
				onFailure?.(error);
			});
			return true;
		} catch (error) {
			this.#reportCommandFailure(command, error);
			onFailure?.(error);
			return false;
		}
	}

	#reportCommandFailure(command: ProjectedTerminalCommand, error: unknown): void {
		const operation = command.kind === 'resize-terminal' ? 'resize' : 'command';
		if (operation === 'resize') this.#lastGeometry = undefined;
		this.#errorEmitter.fire({ error, operation });
	}

	#sendInput(
		command: ProjectedTerminalCommand,
		intent: ProjectedInputPublicationIntent,
		onRejected?: () => void,
		preserveSurfaceState = false,
	): void {
		if (!this.#canAdmitInput()) return;
		this.#inputDispatcher.dispatchSync(() => {
			if (!this.#canAdmitInput()) return;
			try {
				if (!this.#publishCurrentInputCommand(command, intent, preserveSurfaceState)) {
					onRejected?.();
				}
			} finally {
				this.#drainDeferredKeyReleases();
			}
		});
	}

	#publishCurrentInputCommand(
		command: ProjectedTerminalCommand,
		intent: ProjectedInputPublicationIntent,
		preserveSurfaceState = false,
	): boolean {
		if (!this.#canAdmitInput()) return false;
		const inputOperationEpoch = this.#inputRepairEpoch;
		if (
			!preserveSurfaceState &&
			!this.#transitionSurfaceBeforeInput(() => this.#inputOperationIsCurrent(inputOperationEpoch))
		) {
			return false;
		}
		if (
			command.kind !== 'focus-input' &&
			this.#inputFocused &&
			!this.#publishedInputFocused &&
			!this.#publishInputCommand(
				{
					kind: 'focus-input',
					payload: { attachmentId: this.#attachmentId, focused: true },
				},
				ProjectedInputPublicationIntent.held,
			)
		) {
			return false;
		}
		if (!this.#inputOperationIsCurrent(inputOperationEpoch)) return false;
		return this.#publishInputCommand(command, intent);
	}

	#transitionSurfaceBeforeInput(isCurrent: () => boolean): boolean {
		if (!isCurrent()) return false;
		if (this.#historyPhase !== ProjectedHistoryPhase.followingBottom) this.scrollToBottom();
		if (!isCurrent()) return false;
		if (this.#selection.state !== 'none') this.clearSelection();
		return isCurrent();
	}

	#publishInputCommand(
		command: ProjectedTerminalCommand,
		intent: ProjectedInputPublicationIntent,
	): boolean {
		if (!this.#inputEnabled || this.#inputPublicationActive) return false;
		if (
			intent === ProjectedInputPublicationIntent.held &&
			!this.#heldInputCommandIsPublishable(command)
		) {
			return true;
		}
		this.#inputPublicationActive = true;
		let accepted = false;
		try {
			accepted = this.#send(command);
			if (accepted && this.#inputEnabled && intent === ProjectedInputPublicationIntent.held) {
				this.#commitPublishedInputOwnership(command);
			}
			return accepted;
		} finally {
			this.#inputPublicationActive = false;
			this.#resumeDeferredInputRepair();
		}
	}

	#heldInputCommandIsPublishable(command: ProjectedTerminalCommand): boolean {
		switch (command.kind) {
			case 'focus-input': {
				return command.payload.focused || this.#publishedInputFocused;
			}
			case 'key-input': {
				return (
					command.payload.action === KeyAction.press || this.#publishedKeys.has(command.payload.key)
				);
			}
			case 'mouse-input': {
				return (
					command.payload.action !== MouseAction.release ||
					(command.payload.button !== null &&
						this.#publishedMouseButtons.has(command.payload.button))
				);
			}
			default: {
				return true;
			}
		}
	}

	#commitPublishedInputOwnership(command: ProjectedTerminalCommand): void {
		switch (command.kind) {
			case 'focus-input': {
				this.#publishedInputFocused = command.payload.focused;
				break;
			}
			case 'key-input': {
				const { action, ...facts } = command.payload;
				if (action === KeyAction.release) this.#publishedKeys.delete(facts.key);
				else this.#publishedKeys.set(facts.key, facts);
				break;
			}
			case 'mouse-input': {
				const { action, button } = command.payload;
				if (action === MouseAction.press && button !== null) {
					this.#publishedMouseButtons.add(button);
				} else if (action === MouseAction.release && button !== null) {
					this.#publishedMouseButtons.delete(button);
				}
				this.#publishedMouseSample =
					this.#publishedMouseButtons.size === 0 ? undefined : command.payload;
				break;
			}
			default:
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

	#installInputListeners(
		root: HTMLElement,
		canvas: HTMLCanvasElement,
		input: HTMLTextAreaElement,
		historyScrollbar: HTMLDivElement,
	): void {
		this.#listen(input, 'keydown', this.#handleKeyDown);
		this.#listen(input, 'keyup', this.#handleKeyUp);
		this.#listen(input, 'beforeinput', this.#handleBeforeInput);
		this.#listen(input, 'compositionstart', this.#handleCompositionStart);
		this.#listen(input, 'compositionupdate', this.#handleCompositionUpdate);
		this.#listen(input, 'compositionend', this.#handleCompositionEnd);
		this.#listen(input, 'paste', this.#handlePaste);
		if (this.#inputEnabled) {
			this.#listen(root, 'dragover', this.#handleDragOver);
			this.#listen(root, 'drop', this.#handleDrop);
		}
		this.#listen(input, 'focus', this.#handleFocus);
		this.#listen(input, 'blur', this.#handleBlur);
		this.#listen(root, 'focusin', this.#handleSurfaceFocusIn);
		this.#listen(root, 'focusout', this.#handleSurfaceFocusOut);
		this.#listen(canvas, 'pointerdown', this.#handlePointerDown);
		this.#listen(canvas, 'pointerup', this.#handlePointerUp);
		this.#listen(canvas, 'pointermove', this.#handlePointerMove);
		this.#listen(canvas, 'pointercancel', this.#handlePointerCancel);
		this.#listen(canvas, 'lostpointercapture', this.#handleLostPointerCapture);
		this.#listen(canvas, 'pointerleave', this.#handlePointerLeave);
		this.#listen(canvas, 'touchend', this.#handleTouchEnd, { passive: false });
		this.#listen(canvas, 'click', this.#handleClick);
		this.#listen(canvas, 'wheel', this.#handleWheel, { passive: false });
		this.#listen(historyScrollbar, 'pointerdown', this.#handlePointerDown);
		this.#listen(historyScrollbar, 'pointerup', this.#handlePointerUp);
		this.#listen(historyScrollbar, 'pointermove', this.#handlePointerMove);
		this.#listen(historyScrollbar, 'pointercancel', this.#handlePointerCancel);
		this.#listen(historyScrollbar, 'lostpointercapture', this.#handleLostPointerCapture);
		this.#listen(historyScrollbar, 'pointerleave', this.#handlePointerLeave);
		this.#listen(historyScrollbar, 'touchend', this.#handleTouchEnd, { passive: false });
		this.#listen(historyScrollbar, 'click', this.#handleClick);
		this.#listen(historyScrollbar, 'wheel', this.#handleWheel, { passive: false });
		const document = canvas.ownerDocument;
		document.addEventListener('pointermove', this.#handleDocumentPointerMove);
		document.addEventListener('pointerup', this.#handleDocumentPointerUp);
		document.addEventListener('visibilitychange', this.#handleVisibilityChange);
		this.#listenerDisposers.push(() => {
			document.removeEventListener('pointermove', this.#handleDocumentPointerMove);
			document.removeEventListener('pointerup', this.#handleDocumentPointerUp);
			document.removeEventListener('visibilitychange', this.#handleVisibilityChange);
		});
		const view = document.defaultView;
		view?.addEventListener('blur', this.#handleWindowBlur);
		this.#listenerDisposers.push(() => view?.removeEventListener('blur', this.#handleWindowBlur));
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
		if (!this.#inputAdmissionIsOpen()) return;
		if (this.#inputEnabled && !this.#canAdmitInput()) return;
		const trackedCode = normalizedKeyboardCode(event.code);
		this.#trackModifier(event.code, true);
		this.#refreshHoveredLinkModifiers(event);
		if (!this.#inputEnabled) {
			if (!event.isComposing && event.keyCode !== 229) {
				this.#handleLocalHistoryKey(event, trackedCode);
			}
			return;
		}
		const inputOperationEpoch = this.#inputRepairEpoch;
		if (!event.isComposing && event.keyCode !== 229) {
			this.#inputState = supersedeCancelledTerminalComposition(this.#inputState);
		}
		if (
			this.#inputState.compositionPhase !== TerminalCompositionPhase.idle ||
			event.isComposing ||
			event.keyCode === 229
		) {
			return;
		}
		if (this.#handleLocalHistoryKey(event, trackedCode)) return;
		if ((event.ctrlKey || event.metaKey) && event.code === 'KeyV') return;
		if (this.#surfaceMousePlatform === 'macos' && event.metaKey) return;
		const facts = this.#keyFacts(event);
		if (!facts) return;
		if (!this.#inputOperationIsCurrent(inputOperationEpoch)) {
			event.preventDefault();
			event.stopPropagation();
			return;
		}
		const previousFacts = this.#pressedKeys.get(trackedCode);
		const previousInputState = this.#inputState;
		this.#pressedKeys.set(trackedCode, facts);
		if (facts.text) {
			this.#inputState = recordTerminalKeyInput(this.#inputState, {
				text: facts.text,
				time: projectedNow(),
			});
		}
		const stagedInputState = this.#inputState;
		this.#sendKey(
			facts,
			event.repeat ? KeyAction.repeat : KeyAction.press,
			ProjectedInputPublicationIntent.held,
			() => {
				if (!this.#inputOperationIsCurrent(inputOperationEpoch)) return;
				if (this.#pressedKeys.get(trackedCode) === facts) {
					if (previousFacts === undefined) this.#pressedKeys.delete(trackedCode);
					else this.#pressedKeys.set(trackedCode, previousFacts);
				}
				if (this.#inputState === stagedInputState) this.#inputState = previousInputState;
			},
		);
		event.preventDefault();
		event.stopPropagation();
	};

	readonly #handleKeyUp = (event: KeyboardEvent): void => {
		if (!this.#inputAdmissionIsOpen()) return;
		const trackedCode = normalizedKeyboardCode(event.code);
		const pressedFacts = this.#pressedKeys.get(trackedCode);
		const eventKey = terminalKeyCode(trackedCode, keyboardKeyText(event.key));
		const ownedFacts =
			pressedFacts ?? (eventKey === null ? undefined : this.#publishedKeys.get(eventKey));
		this.#pressedKeys.delete(trackedCode);
		this.#trackModifier(event.code, false);
		this.#refreshHoveredLinkModifiers(event);
		if (!this.#inputEnabled) return;
		if (ownedFacts) {
			const eventFacts = this.#keyFacts(event);
			const facts: KeyFacts =
				eventFacts === null
					? {
							...ownedFacts,
							consumedModifiers: emptyModifiers(),
							modifiers: this.#modifiers(event),
							text: '',
						}
					: { ...eventFacts, key: ownedFacts.key };
			this.#publishOrDeferKeyRelease(facts);
			event.preventDefault();
			event.stopPropagation();
		}
	};

	#handleLocalHistoryKey(event: KeyboardEvent, code: string): boolean {
		if (!event.shiftKey || event.altKey || event.ctrlKey || event.metaKey) return false;
		if (code === 'PageUp' || code === 'NumpadPageUp') {
			this.scrollPages(-1);
		} else if (code === 'PageDown' || code === 'NumpadPageDown') {
			this.scrollPages(1);
		} else {
			return false;
		}
		event.preventDefault();
		event.stopPropagation();
		return true;
	}

	#publishOrDeferKeyRelease(facts: KeyFacts): void {
		if (this.#inputPublicationActive) {
			this.#deferredKeyReleases.set(facts.key, facts);
			return;
		}
		this.#sendKey(facts, KeyAction.release, ProjectedInputPublicationIntent.held);
	}

	#drainDeferredKeyReleases(): void {
		if (this.#inputPublicationActive || this.#deferredKeyReleases.size === 0) return;
		const releases = [...this.#deferredKeyReleases.values()];
		this.#deferredKeyReleases.clear();
		for (const facts of releases) {
			if (!this.#inputAdmissionIsOpen() || !this.#inputEnabled) return;
			this.#sendKey(facts, KeyAction.release, ProjectedInputPublicationIntent.held);
		}
	}

	#sendKey(
		facts: KeyFacts,
		action: KeyInput['action'],
		intent: ProjectedInputPublicationIntent,
		onRejected?: () => void,
	): void {
		this.#sendInput(
			{
				kind: 'key-input',
				payload: { action, attachmentId: this.#attachmentId, ...facts },
			},
			intent,
			onRejected,
			action === KeyAction.release,
		);
	}

	#keyFacts(event: KeyboardEvent): KeyFacts | null {
		const trackedCode = normalizedKeyboardCode(event.code);
		const text = keyboardKeyText(event.key);
		const code = terminalKeyCode(trackedCode, text);
		if (!code) return null;
		const unshiftedCodepoint = unshiftedCharacter(trackedCode)?.codePointAt(0) ?? 0;
		const modifiers = this.#modifiers(event);
		const consumedModifiers = emptyModifiers();
		if (event.shiftKey && text && unshiftedCodepoint !== text.codePointAt(0)) {
			consumedModifiers.shift = true;
			consumedModifiers.shiftRight = modifiers.shiftRight;
		}
		if (text && event.getModifierState?.('AltGraph')) {
			consumedModifiers.alt = modifiers.alt;
			consumedModifiers.altRight = modifiers.altRight;
			consumedModifiers.control = modifiers.control;
			consumedModifiers.controlRight = modifiers.controlRight;
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
		code = normalizedKeyboardCode(code);
		if (!['AltRight', 'ControlRight', 'MetaRight', 'ShiftRight'].includes(code)) return;
		if (pressed) this.#pressedModifierCodes.add(code);
		else this.#pressedModifierCodes.delete(code);
	}

	#refreshHoveredLinkModifiers(event: KeyboardEvent): void {
		this.#setSurfaceMouseModifiers(event);
		const point = this.#hoveredLinkPoint;
		if (point === undefined) return;
		this.#presentHoveredLink(this.#hoveredLink, {
			...point,
			control: event.ctrlKey,
			meta: event.metaKey,
		});
	}

	readonly #handleBeforeInput = (event: InputEvent): void => {
		if (this.#repairReadOnlyInputEvent(event)) return;
		if (!this.#canAdmitInput()) return;
		const data = event.data ?? '';
		const origin = isCompositionBeforeInput(event)
			? TerminalBeforeInputOrigin.composition
			: TerminalBeforeInputOrigin.independent;
		if (
			origin === TerminalBeforeInputOrigin.composition &&
			this.#inputState.compositionPhase !== TerminalCompositionPhase.eligible &&
			(this.#inputState.compositionPhase === TerminalCompositionPhase.cancelledAwaitingEnd ||
				this.#inputState.compositionSettlement !== undefined)
		) {
			this.#inputState = observeTerminalBeforeInput(
				this.#inputState,
				data,
				projectedNow(),
				INPUT_DEDUPLICATION_MS,
				origin,
			).state;
			event.preventDefault();
			event.stopPropagation();
			this.#clearInput();
			return;
		}
		if (origin === TerminalBeforeInputOrigin.independent) {
			this.#inputState = supersedeCancelledTerminalComposition(this.#inputState);
		}
		if (
			event.isComposing &&
			this.#inputState.compositionPhase === TerminalCompositionPhase.eligible
		) {
			return;
		}
		if (
			this.#inputState.compositionPhase === TerminalCompositionPhase.eligible &&
			!isTextInsertionInputType(event.inputType)
		) {
			return;
		}
		if (event.inputType === 'insertFromPaste' && data) {
			event.preventDefault();
			event.stopPropagation();
			const decision = observeTerminalPaste(
				this.#inputState,
				data,
				'beforeinput',
				projectedNow(),
				INPUT_DEDUPLICATION_MS,
			);
			this.#inputState = decision.state;
			if (!decision.duplicate) {
				this.#observeDomPaste(this.paste(data, 'beforeinput'));
				this.#inputState = recordTerminalPaste(this.#inputState, {
					source: 'beforeinput',
					text: data,
					time: projectedNow(),
				});
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
		if (!isTextInsertionInputType(event.inputType)) return;
		const text = data;
		if (!text) {
			event.preventDefault();
			return;
		}
		const now = projectedNow();
		const compositionCommit =
			this.#inputState.compositionPhase === TerminalCompositionPhase.eligible;
		const decision = observeTerminalBeforeInput(
			this.#inputState,
			text,
			now,
			INPUT_DEDUPLICATION_MS,
			origin,
		);
		this.#inputState = decision.state;
		const publicationEpoch = this.#inputRepairEpoch;
		const publicationState = decision.state;
		if (!decision.publishText) {
			event.preventDefault();
			event.stopPropagation();
			this.#clearInput();
			return;
		}
		const sent = this.#sendText(text);
		event.preventDefault();
		event.stopPropagation();
		if (!this.#inputOperationIsCurrent(publicationEpoch) || this.#inputState !== publicationState) {
			return;
		}
		if (sent && compositionCommit) this.#clearCompositionPresentation();
		else this.#clearInput();
	};

	readonly #handleCompositionStart = (event: CompositionEvent): void => {
		if (this.#repairReadOnlyInputEvent(event)) return;
		if (!this.#canAdmitInput() && !this.#canStageRefocusedComposition()) return;
		const inputOperationEpoch = this.#inputRepairEpoch;
		const nextState = beginTerminalComposition(this.#inputState);
		if (nextState === this.#inputState) return;
		this.#inputState = nextState;
		if (
			!this.#transitionSurfaceBeforeInput(() =>
				this.#compositionOperationIsCurrent(inputOperationEpoch),
			)
		) {
			return;
		}
		this.#updatePreedit(event.data ?? '');
	};

	readonly #handleCompositionUpdate = (event: CompositionEvent): void => {
		if (this.#repairReadOnlyInputEvent(event)) return;
		if (!this.#canAdmitInput() && !this.#canStageRefocusedComposition()) return;
		if (!terminalCompositionUpdateIsEligible(this.#inputState)) {
			if (this.#inputState.compositionPhase === TerminalCompositionPhase.cancelledAwaitingEnd) {
				this.#clearInput();
			}
			return;
		}
		this.#updatePreedit(event.data ?? '');
	};

	readonly #handleCompositionEnd = (event: CompositionEvent): void => {
		if (this.#repairReadOnlyInputEvent(event)) return;
		if (!this.#canAdmitInput()) return;
		const wasComposing = terminalCompositionUpdateIsEligible(this.#inputState);
		const data = event.data ?? '';
		const decision = finishTerminalComposition(
			this.#inputState,
			data,
			projectedNow(),
			INPUT_DEDUPLICATION_MS,
		);
		if (decision.state === this.#inputState) return;
		this.#inputState = decision.state;
		this.#clearCompositionPresentation();
		if (decision.publishText && !shouldSuppressComposingControlInput(data, wasComposing)) {
			this.#sendText(data);
		}
	};

	#repairReadOnlyInputEvent(event: Event): boolean {
		if (this.#inputEnabled) return false;
		event.preventDefault();
		event.stopPropagation();
		this.#inputState = repairTerminalInputState(this.#inputState).state;
		this.#clearCompositionPresentation();
		return true;
	}

	#clearCompositionPresentation(): void {
		this.#preeditState = undefined;
		this.#preeditText = '';
		this.#preeditCaretUtf16Offset = 0;
		this.#syncPreeditPresentation();
		this.#preeditCellWidth = 0;
		this.#preeditCaretPixelOffset = 0;
		this.#updateInputPosition();
		this.#clearInput();
	}

	#updatePreedit(data: string): void {
		const input = this.#input;
		const selection = {
			start: input?.selectionStart ?? data.length,
			end: input?.selectionEnd ?? data.length,
		};
		const state = buildPreeditState(data, selection, (codepoint) =>
			ghosttyCodepointWidth(codepoint, eastAsianWidth),
		);
		this.#preeditState = state;
		this.#preeditText = data;
		this.#preeditCellWidth = state.width;
		const caretOffset = Math.max(0, Math.min(data.length, input?.selectionEnd ?? data.length));
		this.#preeditCaretUtf16Offset = caretOffset;
		this.#preeditCaretPixelOffset = data
			? (this.#renderer?.measureTextWidth(data.slice(0, caretOffset)) ??
				state.selection.end * (this.#renderer?.getMetrics().width ?? 0))
			: 0;
		this.#syncPreeditPresentation(true);
	}

	#refreshPreeditGeometry(): void {
		const state = this.#preeditState;
		if (state === undefined) {
			this.#preeditCaretPixelOffset = 0;
			this.#updateInputPosition();
			return;
		}
		this.#preeditCaretPixelOffset =
			this.#renderer?.measureTextWidth(this.#preeditText.slice(0, this.#preeditCaretUtf16Offset)) ??
			state.selection.end * (this.#renderer?.getMetrics().width ?? 0);
		this.#syncPreeditPresentation(true);
	}

	#syncPreeditPresentation(force = false): void {
		const state = this.#preeditState;
		const shouldPresent = state !== undefined && this.#canPresentLiveCursor();
		if (shouldPresent && state !== undefined) {
			if (force || !this.#preeditPresented) {
				this.#renderer?.setPreedit(state.cells, state.selection);
			}
			this.#preeditPresented = true;
		} else if (this.#preeditPresented) {
			this.#renderer?.setPreedit([]);
			this.#preeditPresented = false;
		}
		this.#updateInputPosition();
	}

	#canPresentLiveCursor(): boolean {
		return (
			this.#historyPhase === ProjectedHistoryPhase.followingBottom &&
			this.#store.metadata?.cursor.visible === true
		);
	}

	#updateInputPosition(): void {
		const input = this.#input;
		const root = this.#root;
		const canvas = this.#canvas;
		const renderer = this.#renderer;
		if (!input || !root || !canvas || !renderer) return;
		const metadata = this.#canPresentLiveCursor() ? this.#store.metadata : null;
		if (metadata === null) {
			input.style.left = '0px';
			input.style.top = '0px';
			input.style.width = '1px';
			input.style.height = '1px';
			input.style.lineHeight = '1px';
			return;
		}
		const rootBounds = root.getBoundingClientRect();
		const canvasBounds = canvas.getBoundingClientRect();
		const metrics = renderer.getMetrics();
		const preeditLayout =
			this.#preeditState === undefined
				? null
				: projectPreeditLayout(
						this.#preeditState.cells,
						this.#preeditState.selection,
						{ column: metadata.cursor.column, row: metadata.cursor.row },
						metadata.columns,
						metadata.rows,
					);
		const anchor = calculateInputAnchor({
			canvasLeft: canvasBounds.left,
			canvasTop: canvasBounds.top,
			cellHeight: metrics.height,
			cellWidth: metrics.width,
			columns: metadata.columns,
			cursorColumn: preeditLayout?.caret.column ?? metadata.cursor.column,
			cursorRow: preeditLayout?.caret.row ?? metadata.cursor.row,
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

	#sendText(text: string): boolean {
		if (textEncoder.encode(text).byteLength > PROTOCOL_LIMITS.maxInputBytes) return false;
		this.#sendInput(
			{
				kind: 'text-input',
				payload: { attachmentId: this.#attachmentId, text },
			},
			ProjectedInputPublicationIntent.stateless,
		);
		return true;
	}

	#sendSyntheticKey(
		key: TerminalKeyCodeValue,
		modifiers: TerminalModifiers = emptyModifiers(),
		text = '',
		unshiftedCodepoint = 0,
	): void {
		const facts: KeyFacts = {
			composing: false,
			consumedModifiers: emptyModifiers(),
			key,
			modifiers,
			text,
			unshiftedCodepoint,
		};
		this.#sendKey(facts, KeyAction.press, ProjectedInputPublicationIntent.held);
		this.#sendKey(facts, KeyAction.release, ProjectedInputPublicationIntent.held);
	}

	readonly #handlePaste = (event: ClipboardEvent): void => {
		if (this.#repairReadOnlyInputEvent(event)) return;
		if (!this.#canAdmitInput()) return;
		event.preventDefault();
		event.stopPropagation();
		const clipboardData = event.clipboardData;
		const text = clipboardData?.getData('text/plain') ?? '';
		if (!text) {
			if (clipboardContainsImage(clipboardData)) {
				this.#sendSyntheticKey('KeyV', { ...emptyModifiers(), control: true }, 'v', 0x76);
			}
			return;
		}
		const decision = observeTerminalPaste(
			this.#inputState,
			text,
			'clipboard',
			projectedNow(),
			INPUT_DEDUPLICATION_MS,
		);
		this.#inputState = decision.state;
		if (decision.duplicate) return;
		this.#observeDomPaste(this.paste(text, 'clipboard'));
		this.#inputState = recordTerminalPaste(this.#inputState, {
			source: 'clipboard',
			text,
			time: projectedNow(),
		});
	};

	readonly #handleDragOver = (event: DragEvent): void => {
		if (!this.#canAdmitInput()) return;
		const transfer = event.dataTransfer;
		if (transfer === null || !this.#supportsPlainTextDrop(transfer)) return;
		event.preventDefault();
		transfer.dropEffect = 'copy';
	};

	readonly #handleDrop = (event: DragEvent): void => {
		if (!this.#canAdmitInput()) return;
		const transfer = event.dataTransfer;
		if (transfer === null || !this.#supportsPlainTextDrop(transfer)) return;
		event.preventDefault();
		event.stopPropagation();
		this.#observeDomPaste(this.paste(transfer.getData('text/plain'), 'drop'));
	};

	#observeDomPaste(result: Promise<ProjectedPasteResult>): void {
		void result.catch((error) => this.#errorEmitter.fire({ error, operation: 'command' }));
	}

	#supportsPlainTextDrop(transfer: DataTransfer): boolean {
		return transfer.files.length === 0 && [...transfer.types].includes('text/plain');
	}

	readonly #handleFocus = (): void => {
		if (this.#inputAdmission === ProjectedInputAdmission.disposed || this.#isDisposed()) {
			return;
		}
		this.#inputState = supersedeCancelledTerminalComposition(this.#inputState);
		this.#renderer?.setFocused(true);
		this.#updateInputPosition();
		if (this.#inputAdmission === ProjectedInputAdmission.repairing && this.#inputEnabled) {
			this.#inputFocused = true;
			this.#inputRefocusAfterRepair = true;
			return;
		}
		if (!this.#inputEnabled || this.#inputFocused) return;
		this.#inputFocused = true;
		this.#sendInput(
			{
				kind: 'focus-input',
				payload: { attachmentId: this.#attachmentId, focused: true },
			},
			ProjectedInputPublicationIntent.held,
			undefined,
			true,
		);
	};

	#focusTargetIsWithinSurface(target: EventTarget | null): boolean {
		const root = this.#root;
		const NodeConstructor = root?.ownerDocument.defaultView?.Node;
		return (
			root !== undefined &&
			NodeConstructor !== undefined &&
			target instanceof NodeConstructor &&
			root.contains(target)
		);
	}

	readonly #handleBlur = (event: FocusEvent): void => {
		if (this.#focusTargetIsWithinSurface(event.relatedTarget)) return;
		this.#renderer?.setFocused(false);
		this.#repairInputState();
	};

	readonly #handleSurfaceFocusIn = (event: FocusEvent): void => {
		if (event.target === this.#input || this.#focusTargetIsWithinSurface(event.relatedTarget)) {
			return;
		}
		this.#handleFocus();
	};

	readonly #handleSurfaceFocusOut = (event: FocusEvent): void => {
		if (event.target === this.#input || this.#focusTargetIsWithinSurface(event.relatedTarget)) {
			return;
		}
		this.#handleBlur(event);
	};

	readonly #handleWindowBlur = (): void => {
		this.#renderer?.setFocused(false);
		this.#repairInputState();
	};

	readonly #handleVisibilityChange = (): void => {
		this.#documentHidden = this.#root?.ownerDocument.visibilityState === 'hidden';
		this.#syncRendererSuspension();
		if (this.#documentHidden) this.#handleWindowBlur();
	};

	#repairInputState(): void {
		if (!this.#beginInputRepair()) {
			if (this.#inputRepairTransaction !== undefined) {
				this.#stageActiveInputRepair(false, true);
			}
			return;
		}
		this.#stageActiveInputRepair(false, true);
		this.#dispatchInputRepair();
	}

	#beginInputRepair(): boolean {
		if (this.#inputAdmission !== ProjectedInputAdmission.accepting) return false;
		this.#inputAdmission = ProjectedInputAdmission.repairing;
		this.#inputRepairEpoch++;
		this.#inputDispatcher.invalidate();
		this.#inputRepairTransaction = this.#createInputRepairTransaction();
		return true;
	}

	#createInputRepairTransaction(): ProjectedInputRepairTransaction {
		return {
			afterCleanup: undefined,
			attemptedKeys: new Set(),
			attemptedMouseButtons: new Set(),
			commands: [],
			deferredDispose: false,
			deferredFocusLoss: false,
			dispatchRequested: false,
			focusOutCommand: undefined,
			focusOutSent: false,
			nextCommandIndex: 0,
			transientDisposed: false,
			transientRepaired: false,
		};
	}

	#canAdmitInput(): boolean {
		return this.#inputAdmissionIsOpen() && !this.#inputPublicationActive && this.#inputEnabled;
	}

	#inputAdmissionIsOpen(): boolean {
		return this.#inputAdmission === ProjectedInputAdmission.accepting && !this.#isDisposed();
	}

	#inputOperationIsCurrent(epoch: number): boolean {
		return epoch === this.#inputRepairEpoch && this.#inputAdmissionIsOpen();
	}

	#canStageRefocusedComposition(): boolean {
		return (
			this.#inputAdmission === ProjectedInputAdmission.repairing &&
			this.#inputRefocusAfterRepair &&
			!this.#disposeAfterRepair &&
			!this.#disposalStarted
		);
	}

	#compositionOperationIsCurrent(epoch: number): boolean {
		return (
			epoch === this.#inputRepairEpoch &&
			(this.#inputAdmissionIsOpen() || this.#canStageRefocusedComposition())
		);
	}

	#stageActiveInputRepair(dispose: boolean, focusLoss: boolean): void {
		const transaction = this.#inputRepairTransaction;
		if (transaction === undefined) return;
		this.#repairLocalInputState(transaction, dispose, focusLoss);
		if (this.#inputPublicationActive) {
			transaction.deferredDispose ||= dispose;
			transaction.deferredFocusLoss ||= focusLoss;
			return;
		}
		this.#stagePublishedInputRepair(transaction, focusLoss);
	}

	#repairLocalInputState(
		transaction: ProjectedInputRepairTransaction,
		dispose: boolean,
		focusLoss: boolean,
	): void {
		if (!transaction.transientRepaired || (dispose && !transaction.transientDisposed)) {
			this.#repairTransientInputState(dispose);
			transaction.transientRepaired = true;
			transaction.transientDisposed ||= dispose;
		}
		this.#deferredKeyReleases.clear();
		this.#pressedKeys.clear();
		this.#finishApplicationMouseGesture(true);
		this.#pressedMouseButtons.clear();
		this.#pressedModifierCodes.clear();
		this.#resetSurfaceMouseModifiers();
		this.#clearApplicationMouseClickSuppression();
		if (!focusLoss) return;
		this.#inputFocused = false;
		this.#inputRefocusAfterRepair = false;
		this.#clearHistoryScrollbarClickSuppression();
		this.#finishHistoryScrollbarGesture(true, !dispose);
		this.#cancelSelectionGesture(true);
	}

	#stagePublishedInputRepair(
		transaction: ProjectedInputRepairTransaction,
		focusLoss: boolean,
	): void {
		this.#queuePublishedReleaseCommands(transaction);
		if (
			focusLoss &&
			this.#publishedInputFocused &&
			this.#inputEnabled &&
			!transaction.focusOutSent
		) {
			transaction.focusOutCommand = {
				kind: 'focus-input',
				payload: { attachmentId: this.#attachmentId, focused: false },
			};
		}
	}

	#dispatchInputRepair(afterCleanup?: () => void): void {
		const transaction = this.#inputRepairTransaction;
		if (transaction === undefined) return;
		if (afterCleanup !== undefined) {
			const prior = transaction.afterCleanup;
			transaction.afterCleanup =
				prior === undefined
					? afterCleanup
					: () => {
							prior();
							afterCleanup();
						};
		}
		transaction.dispatchRequested = true;
		this.#resumeDeferredInputRepair();
	}

	#resumeDeferredInputRepair(): void {
		if (this.#inputPublicationActive) return;
		const transaction = this.#inputRepairTransaction;
		if (transaction === undefined) return;
		if (transaction.deferredDispose || transaction.deferredFocusLoss) {
			const focusLoss = transaction.deferredFocusLoss;
			transaction.deferredDispose = false;
			transaction.deferredFocusLoss = false;
			this.#stagePublishedInputRepair(transaction, focusLoss);
		}
		if (!transaction.dispatchRequested || this.#inputRepairDraining) return;
		transaction.dispatchRequested = false;
		this.#inputDispatcher.dispatchSync(() => {
			if (this.#inputRepairTransaction !== transaction) return;
			this.#inputRepairDraining = true;
			try {
				this.#drainInputRepairTransaction(transaction);
				const afterCleanup = transaction.afterCleanup;
				transaction.afterCleanup = undefined;
				afterCleanup?.();
				this.#drainInputRepairTransaction(transaction);
			} finally {
				this.#inputRepairDraining = false;
				this.#inputRepairTransaction = undefined;
				if (this.#disposeAfterRepair) {
					this.#disposeAfterRepair = false;
					this.#performDispose(true);
				} else if (this.#inputAdmission === ProjectedInputAdmission.repairing) {
					this.#inputAdmission = ProjectedInputAdmission.accepting;
					this.#inputRefocusAfterRepair = false;
					if (this.#inputFocused && !this.#publishedInputFocused) {
						this.#sendInput(
							{
								kind: 'focus-input',
								payload: { attachmentId: this.#attachmentId, focused: true },
							},
							ProjectedInputPublicationIntent.held,
							undefined,
							true,
						);
					}
				}
			}
		});
	}

	#drainInputRepairTransaction(transaction: ProjectedInputRepairTransaction): void {
		while (transaction.nextCommandIndex < transaction.commands.length) {
			const command = transaction.commands[transaction.nextCommandIndex];
			transaction.nextCommandIndex++;
			if (command !== undefined) {
				this.#publishInputCommand(command, ProjectedInputPublicationIntent.held);
			}
		}
		const focusOutCommand = transaction.focusOutCommand;
		if (focusOutCommand === undefined || transaction.focusOutSent) return;
		transaction.focusOutSent = true;
		this.#publishInputCommand(focusOutCommand, ProjectedInputPublicationIntent.held);
	}

	#dispatchDisposedInputCleanup(commands: readonly ProjectedTerminalCommand[]): void {
		if (commands.length === 0) return;
		this.#inputDispatcher.dispatchSync(() => {
			for (const command of commands) {
				this.#publishInputCommand(command, ProjectedInputPublicationIntent.held);
			}
		});
	}

	#prepareFocusLossInputRepair(dispose: boolean): ProjectedTerminalCommand[] {
		this.#repairTransientInputState(dispose);
		const commands = [
			...this.#publishedKeyReleaseCommands(),
			...this.#publishedMouseReleaseCommands(),
		];
		this.#deferredKeyReleases.clear();
		this.#pressedKeys.clear();
		this.#finishApplicationMouseGesture(true);
		this.#pressedMouseButtons.clear();
		this.#inputFocused = false;
		this.#pressedModifierCodes.clear();
		this.#resetSurfaceMouseModifiers();
		this.#clearApplicationMouseClickSuppression();
		this.#clearHistoryScrollbarClickSuppression();
		this.#finishHistoryScrollbarGesture(true, !dispose);
		this.#cancelSelectionGesture(true);
		if (!this.#inputEnabled) return [];
		if (this.#publishedInputFocused) {
			commands.push({
				kind: 'focus-input',
				payload: { attachmentId: this.#attachmentId, focused: false },
			});
		}
		return commands;
	}

	#repairTransientInputState(dispose = false): void {
		const repair = dispose
			? disposeTerminalInputState(this.#inputState)
			: repairTerminalInputState(this.#inputState);
		this.#inputState = repair.state;
		this.#clearCompositionPresentation();
	}

	#queuePublishedReleaseCommands(transaction: ProjectedInputRepairTransaction): void {
		const pendingCommands = transaction.commands.slice(transaction.nextCommandIndex);
		const pendingKeys = new Set<TerminalKeyCodeValue>();
		const pendingMouseButtons = new Set<Exclude<MouseInput['button'], null>>();
		for (const command of pendingCommands) {
			if (command.kind === 'key-input' && command.payload.action === KeyAction.release) {
				pendingKeys.add(command.payload.key);
			} else if (
				command.kind === 'mouse-input' &&
				command.payload.action === MouseAction.release &&
				command.payload.button !== null
			) {
				pendingMouseButtons.add(command.payload.button);
			}
		}
		const commands = [
			...this.#publishedKeyReleaseCommands().filter(
				(command) =>
					command.kind === 'key-input' &&
					!pendingKeys.has(command.payload.key) &&
					!transaction.attemptedKeys.has(command.payload.key),
			),
			...this.#publishedMouseReleaseCommands().filter(
				(command) =>
					command.kind === 'mouse-input' &&
					command.payload.button !== null &&
					!pendingMouseButtons.has(command.payload.button) &&
					!transaction.attemptedMouseButtons.has(command.payload.button),
			),
		];
		for (const command of commands) {
			if (command.kind === 'key-input') {
				transaction.attemptedKeys.add(command.payload.key);
			} else if (command.kind === 'mouse-input' && command.payload.button !== null) {
				transaction.attemptedMouseButtons.add(command.payload.button);
			}
		}
		transaction.commands.splice(transaction.nextCommandIndex, 0, ...commands);
		this.#pressedKeys.clear();
		this.#pressedMouseButtons.clear();
	}

	#publishedKeyReleaseCommands(): ProjectedTerminalCommand[] {
		return [...this.#publishedKeys.values()].map(
			(facts): ProjectedTerminalCommand => ({
				kind: 'key-input',
				payload: { action: KeyAction.release, attachmentId: this.#attachmentId, ...facts },
			}),
		);
	}

	#publishedMouseReleaseCommands(): ProjectedTerminalCommand[] {
		const last = this.#publishedMouseSample;
		if (last === undefined) return [];
		const commands: ProjectedTerminalCommand[] = [];
		let pressedButtons = mouseButtonSetMask(this.#publishedMouseButtons);
		for (const button of this.#publishedMouseButtons) {
			pressedButtons &= ~(1 << (button - 1));
			commands.push({
				kind: 'mouse-input',
				payload: {
					...last,
					action: MouseAction.release,
					button,
					pressedButtons,
				},
			});
		}
		return commands;
	}

	#startApplicationMouseGesture(event: PointerEvent): boolean {
		const gesture = this.#applicationMouseGesture;
		if (gesture !== undefined) return gesture.pointerId === event.pointerId;
		const canvas = this.#canvas;
		if (canvas === undefined) return false;
		this.#applicationMouseGesture = { pointerId: event.pointerId };
		this.#updateSurfaceMouseCursor();
		try {
			canvas.setPointerCapture(event.pointerId);
		} catch {
			// Focus and visibility repair still release Host-published mouse ownership.
		}
		return true;
	}

	#finishApplicationMouseGesture(releaseCapture: boolean): void {
		const gesture = this.#applicationMouseGesture;
		if (gesture === undefined) return;
		this.#applicationMouseGesture = undefined;
		this.#updateSurfaceMouseCursor();
		if (!releaseCapture) return;
		try {
			this.#canvas?.releasePointerCapture(gesture.pointerId);
		} catch {
			// Browsers may release capture before the terminal repairs the gesture.
		}
	}

	#applicationMouseGestureMatches(event: PointerEvent): boolean {
		const gesture = this.#applicationMouseGesture;
		return gesture !== undefined && gesture.pointerId === event.pointerId;
	}

	#repairApplicationMouseGesture(event: PointerEvent, releaseCapture: boolean): boolean {
		if (!this.#applicationMouseGestureMatches(event)) return false;
		const buttons = new Set([...this.#pressedMouseButtons, ...this.#publishedMouseButtons]);
		let pressedButtons = mouseButtonSetMask(buttons);
		const commands: ProjectedTerminalCommand[] = [];
		for (const button of buttons) {
			pressedButtons &= ~(1 << (button - 1));
			const payload =
				this.#mousePayload(event, MouseAction.release, button, pressedButtons) ??
				mouseReleasePayload(
					this.#inputState.mouse ?? this.#publishedMouseSample,
					button,
					pressedButtons,
				);
			if (payload !== null) commands.push({ kind: 'mouse-input', payload });
		}
		this.#pressedMouseButtons.clear();
		this.#inputState = recordTerminalMouseSample(this.#inputState, undefined);
		this.#finishApplicationMouseGesture(releaseCapture);
		this.#clearApplicationMouseClickSuppression();
		for (const command of commands) {
			this.#sendInput(command, ProjectedInputPublicationIntent.held, undefined, true);
		}
		return true;
	}

	#startHistoryScrollbarGesture(event: PointerEvent): boolean {
		if (event.button !== 0) return false;
		const scrollbar = this.#historyScrollbar;
		const root = this.#root;
		const projection = this.#historyScrollbarProjection();
		if (!scrollbar || !root || projection === null) return false;
		const rectangle = root.getBoundingClientRect();
		const pointerX = event.clientX - rectangle.left;
		const pointerY = event.clientY - rectangle.top;
		if (!historyScrollbarContainsPoint(projection, pointerX, pointerY)) return false;

		this.#cancelSelectionGesture(true);
		this.#clearHoveredLink();
		const start = beginHistoryScrollbarDrag(projection, pointerY);
		this.#historyScrollbarGesture = { drag: start.drag, pointerId: event.pointerId };
		this.#showHistoryScrollbar();
		try {
			scrollbar.setPointerCapture(event.pointerId);
		} catch {
			// Focus and visibility repair still end the drag when capture is unavailable.
		}
		this.#requestHistoryStartRow(start.targetStartRow);
		this.#ownHistoryScrollbarEvent(event);
		return true;
	}

	#finishHistoryScrollbarGesture(releaseCapture: boolean, scheduleFade = true): boolean {
		const gesture = this.#historyScrollbarGesture;
		if (gesture === undefined) return false;
		this.#historyScrollbarGesture = undefined;
		if (scheduleFade) this.#showHistoryScrollbar();
		else this.#cancelHistoryScrollbarFade();
		if (!releaseCapture) return true;
		try {
			this.#historyScrollbar?.releasePointerCapture(gesture.pointerId);
		} catch {
			// Browsers may release capture before the terminal repairs the drag.
		}
		return true;
	}

	#ownHistoryScrollbarEvent(event: Event): void {
		event.preventDefault();
		event.stopImmediatePropagation();
	}

	readonly #handlePointerDown = (event: PointerEvent): void => {
		if (!this.#inputAdmissionIsOpen()) return;
		this.#setSurfaceMouseModifiers(event);
		const inputOperationEpoch = this.#inputRepairEpoch;
		this.#clearApplicationMouseClickSuppression();
		this.#clearHistoryScrollbarClickSuppression();
		if (this.#historyScrollbarGesture !== undefined) {
			this.#selectionClickSequence = undefined;
			this.#clearHoveredLink();
			this.#ownHistoryScrollbarEvent(event);
			return;
		}
		if (event.pointerType !== 'touch') this.focus();
		if (!this.#inputOperationIsCurrent(inputOperationEpoch)) {
			event.preventDefault();
			return;
		}
		if (this.#startHistoryScrollbarGesture(event)) {
			this.#selectionClickSequence = undefined;
			return;
		}
		if (this.#applicationMousePressOwned(event.shiftKey)) {
			this.#selectionClickSequence = undefined;
			this.#clearHoveredLink();
			const button = domMouseButton(event.button);
			if (button === null || !this.#canAdmitInput() || !this.#startApplicationMouseGesture(event)) {
				event.preventDefault();
				return;
			}
			this.#sendMouse(
				event,
				MouseAction.press,
				button,
				ProjectedInputPublicationIntent.held,
				() => {
					if (this.#pressedMouseButtons.size === 0 && this.#publishedMouseButtons.size === 0) {
						this.#finishApplicationMouseGesture(true);
					}
				},
			);
			event.preventDefault();
			return;
		}
		if (event.button !== 0) return;
		this.#stopSelectionAutoscroll();
		const hoveredLink = this.#hoveredLink;
		const location = this.#selectionGestureLocation(event);
		const hoveredPoint = location?.point ?? null;
		if (
			hoveredLink !== undefined &&
			hoveredPoint !== null &&
			(event.ctrlKey || event.metaKey) &&
			terminalLinkContains(hoveredLink, hoveredPoint.column, Number(BigInt(hoveredPoint.row))) &&
			isPermittedExternalTerminalTarget(hoveredLink)
		) {
			this.#selectionClickSequence = undefined;
			event.preventDefault();
			return;
		}
		if (location === null) return;
		const clickCount = this.#nextSelectionClickCount(event, location);
		const gesture = {
			action: 'start' as const,
			clickCount,
			point: location.point,
			position: location.position,
			selectOutput: event.ctrlKey || event.metaKey,
		};
		if (!this.#canStartSelectionGesture()) {
			this.#selectionClickSequence = undefined;
			event.preventDefault();
			return;
		}
		this.#selectionGestureActive = true;
		this.#selectionGestureLast = location.point;
		this.#selectionGesturePointerId = event.pointerId;
		this.#selectionPointer = {
			clientX: event.clientX,
			clientY: event.clientY,
			rectangle: event.altKey,
		};
		this.#updateSurfaceMouseCursor();
		try {
			this.#canvas?.setPointerCapture(event.pointerId);
		} catch {
			// Window-focus and visibility repair still ends a gesture when capture is unavailable.
		}
		this.#startSelectionGesture(gesture);
		event.preventDefault();
	};

	readonly #handleTouchEnd = (event: TouchEvent): void => {
		event.preventDefault();
		if (!this.#inputAdmissionIsOpen()) return;
		this.focus();
	};

	readonly #handlePointerUp = (event: PointerEvent): void => {
		if (!this.#inputAdmissionIsOpen()) return;
		this.#setSurfaceMouseModifiers(event);
		const scrollbarGesture = this.#historyScrollbarGesture;
		if (scrollbarGesture !== undefined) {
			this.#clearHoveredLink();
			if (scrollbarGesture.pointerId === event.pointerId && event.button === 0) {
				this.#armHistoryScrollbarClickSuppression(event);
				this.#finishHistoryScrollbarGesture(true);
			}
			this.#ownHistoryScrollbarEvent(event);
			return;
		}
		if (
			this.#selectionGestureActive &&
			event.button === 0 &&
			(this.#selectionGesturePointerId === undefined ||
				this.#selectionGesturePointerId === event.pointerId)
		) {
			const point = this.#selectionPoint(event);
			if (point !== null && this.#selectionGestureLast !== undefined) {
				this.#selectionGestureLast = point;
			}
			this.#completeSelectionGesture(true);
			event.preventDefault();
			return;
		}
		const button = domMouseButton(event.button);
		if (
			button === null ||
			!this.#applicationMouseGestureMatches(event) ||
			!this.#pressedMouseButtons.has(button)
		) {
			return;
		}
		const inputOperationEpoch = this.#inputRepairEpoch;
		if (button === MouseButton.left) this.#armApplicationMouseClickSuppression(event);
		this.#sendMouse(event, MouseAction.release, button, ProjectedInputPublicationIntent.held);
		if (
			this.#inputOperationIsCurrent(inputOperationEpoch) &&
			this.#pressedMouseButtons.size === 0
		) {
			this.#finishApplicationMouseGesture(true);
		}
		event.preventDefault();
	};

	readonly #handlePointerMove = (event: PointerEvent): void => {
		if (!this.#inputAdmissionIsOpen()) return;
		this.#setSurfaceMouseModifiers(event);
		const scrollbarGesture = this.#historyScrollbarGesture;
		if (scrollbarGesture !== undefined) {
			this.#clearHoveredLink();
			if (scrollbarGesture.pointerId === event.pointerId) {
				if ((event.buttons & 1) === 0) {
					this.#finishHistoryScrollbarGesture(true);
				} else {
					const projection = this.#historyScrollbarProjection();
					const rectangle = this.#root?.getBoundingClientRect();
					if (projection !== null && rectangle !== undefined) {
						this.#requestHistoryStartRow(
							resolveHistoryScrollbarDrag(
								projection,
								scrollbarGesture.drag,
								event.clientY - rectangle.top,
							),
						);
					}
				}
			}
			this.#ownHistoryScrollbarEvent(event);
			return;
		}
		if (
			this.#selectionGestureActive &&
			(event.buttons & 1) === 0 &&
			(this.#selectionGesturePointerId === undefined ||
				this.#selectionGesturePointerId === event.pointerId)
		) {
			this.#clearHoveredLink();
			this.#cancelSelectionGesture(true);
			event.preventDefault();
			return;
		}
		if (
			this.#selectionGestureActive &&
			(event.buttons & 1) !== 0 &&
			(this.#selectionGesturePointerId === undefined ||
				this.#selectionGesturePointerId === event.pointerId)
		) {
			this.#clearHoveredLink();
			this.#updateSelectionAutoscroll({
				clientX: event.clientX,
				clientY: event.clientY,
				rectangle: event.altKey,
			});
			const location = this.#selectionGestureLocation(event);
			if (location !== null) {
				this.#selectionGestureLast = location.point;
				this.#queueSelectionGesture({
					action: 'update',
					geometry: location.geometry,
					point: location.point,
					position: location.position,
					rectangle: event.altKey,
				});
			}
			event.preventDefault();
			return;
		}
		if (event.buttons === 0 && this.#repairApplicationMouseGesture(event, true)) {
			this.#clearHoveredLink();
			event.preventDefault();
			return;
		}
		if (this.#applicationMouseGestureMatches(event) && this.#pressedMouseButtons.size !== 0) {
			this.#clearHoveredLink();
			this.#sendMouse(event, MouseAction.motion, null, ProjectedInputPublicationIntent.held);
			event.preventDefault();
			return;
		}
		if (event.buttons === 0 && this.#applicationMouseHoverOwned(event.shiftKey)) {
			this.#clearHoveredLink();
			this.#sendMouse(event, MouseAction.motion, null, ProjectedInputPublicationIntent.held);
			event.preventDefault();
			return;
		}
		if (this.#applicationMouseTrackingOwnsSurface() && !event.shiftKey) {
			this.#clearHoveredLink();
			return;
		}
		if (event.buttons !== 0) {
			this.#clearHoveredLink();
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
			generation: this.#historyGeneration,
			linkEpoch: this.#linkEpoch,
			meta: event.metaKey,
			revision: this.#historyRevision,
			row: BigInt(point.row),
		});
	};

	readonly #handlePointerLeave = (event: PointerEvent): void => {
		this.#setSurfaceMouseModifiers(event);
		this.#clearHoveredLink();
		if (this.#historyScrollbarGesture !== undefined) this.#ownHistoryScrollbarEvent(event);
	};

	readonly #handleDocumentPointerMove = (event: PointerEvent): void => {
		if (!this.#selectionGestureActive || event.target === this.#canvas) return;
		this.#handlePointerMove(event);
	};

	readonly #handleDocumentPointerUp = (event: PointerEvent): void => {
		if (!this.#selectionGestureActive || event.target === this.#canvas) return;
		this.#handlePointerUp(event);
	};

	readonly #handlePointerCancel = (event: PointerEvent): void => {
		if (!this.#inputAdmissionIsOpen()) return;
		const scrollbarGesture = this.#historyScrollbarGesture;
		if (scrollbarGesture !== undefined) {
			this.#clearHoveredLink();
			if (scrollbarGesture.pointerId === event.pointerId) {
				this.#finishHistoryScrollbarGesture(true);
			}
			this.#ownHistoryScrollbarEvent(event);
			return;
		}
		if (this.#repairApplicationMouseGesture(event, true)) {
			this.#clearHoveredLink();
			event.preventDefault();
			return;
		}
		if (this.#selectionGesturePointerId !== event.pointerId) return;
		this.#cancelSelectionGesture(true);
		event.preventDefault();
	};

	readonly #handleLostPointerCapture = (event: PointerEvent): void => {
		if (!this.#inputAdmissionIsOpen()) return;
		const scrollbarGesture = this.#historyScrollbarGesture;
		if (scrollbarGesture !== undefined) {
			this.#clearHoveredLink();
			if (scrollbarGesture.pointerId === event.pointerId) {
				this.#finishHistoryScrollbarGesture(false);
			}
			this.#ownHistoryScrollbarEvent(event);
			return;
		}
		if (this.#repairApplicationMouseGesture(event, false)) {
			this.#clearHoveredLink();
			return;
		}
		if (this.#selectionGesturePointerId !== event.pointerId) return;
		this.#cancelSelectionGesture(false);
	};

	#completeSelectionGesture(releaseCapture: boolean): void {
		if (!this.#selectionGestureActive) return;
		const canvas = this.#canvas;
		const pointerId = this.#selectionGesturePointerId;
		const gesture = this.#selectionGestureLast;
		this.#selectionGestureActive = false;
		this.#selectionGestureLast = undefined;
		this.#selectionGesturePointerId = undefined;
		this.#stopSelectionAutoscroll();
		this.#updateSurfaceMouseCursor();
		if (gesture !== undefined) this.#queueSelectionGesture({ action: 'end', point: gesture });
		if (!releaseCapture || pointerId === undefined) return;
		try {
			canvas?.releasePointerCapture(pointerId);
		} catch {
			// Browsers may release capture before pointerup is dispatched.
		}
	}

	#cancelSelectionGesture(releaseCapture: boolean): void {
		if (!this.#selectionGestureActive) return;
		const canvas = this.#canvas;
		const pointerId = this.#selectionGesturePointerId;
		this.#selectionGestureActive = false;
		this.#selectionGestureLast = undefined;
		this.#selectionGesturePointerId = undefined;
		this.#selectionClickSequence = undefined;
		this.#stopSelectionAutoscroll();
		this.#updateSurfaceMouseCursor();
		this.#queueSelectionGesture({ action: 'cancel' });
		if (!releaseCapture || pointerId === undefined) return;
		try {
			canvas?.releasePointerCapture(pointerId);
		} catch {
			// Browsers may release capture before cancellation repairs the gesture.
		}
	}

	#cancelSelectionGestureForReset(): void {
		const canvas = this.#canvas;
		const pointerId = this.#selectionGesturePointerId;
		this.#selectionGestureActive = false;
		this.#selectionGestureLast = undefined;
		this.#selectionGesturePointerId = undefined;
		this.#selectionClickSequence = undefined;
		this.#pendingSelectionGestures.length = 0;
		this.#suppressNextSelectionCopy = true;
		this.#cancelScheduledSelectionGesture();
		this.#stopSelectionAutoscroll();
		this.#updateSurfaceMouseCursor();
		if (pointerId === undefined) return;
		try {
			canvas?.releasePointerCapture(pointerId);
		} catch {
			// Browsers may release capture while reset is cancelling the gesture.
		}
	}

	#cancelScheduledSelectionGesture(): void {
		if (this.#selectionGestureHandle?.kind === 'animation-frame') {
			this.#root?.ownerDocument.defaultView?.cancelAnimationFrame(
				this.#selectionGestureHandle.value,
			);
		} else if (this.#selectionGestureHandle?.kind === 'timeout') {
			globalThis.clearTimeout(this.#selectionGestureHandle.value);
		}
		this.#selectionGestureHandle = undefined;
	}

	readonly #handleClick = (event: MouseEvent): void => {
		if (!this.#inputAdmissionIsOpen()) return;
		this.#setSurfaceMouseModifiers(event);
		if (this.#consumeHistoryScrollbarClickSuppression(event)) {
			this.#clearHoveredLink();
			this.#ownHistoryScrollbarEvent(event);
			return;
		}
		if (
			this.#consumeApplicationMouseClickSuppression(event) ||
			(this.#applicationMouseTrackingOwnsSurface() && !event.shiftKey)
		) {
			this.#clearHoveredLink();
			return;
		}
		if (!event.ctrlKey && !event.metaKey) return;
		const point = this.#selectionPoint(event);
		const link = this.#hoveredLink;
		const hoveredPoint = this.#hoveredLinkPoint;
		if (hoveredPoint !== undefined && !this.#linkPointIsCurrent(hoveredPoint)) {
			this.#clearHoveredLink();
			return;
		}
		if (
			point === null ||
			link === undefined ||
			hoveredPoint === undefined ||
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
		if (!this.#inputAdmissionIsOpen()) return;
		this.#setSurfaceMouseModifiers(event);
		if (this.#historyScrollbarGesture !== undefined) {
			this.#clearHoveredLink();
			this.#ownHistoryScrollbarEvent(event);
			return;
		}
		this.#clearHistoryScrollbarClickSuppression();
		const deltaX = Number.isFinite(event.deltaX) ? event.deltaX : 0;
		const deltaY = Number.isFinite(event.deltaY) ? event.deltaY : 0;
		if (deltaX === 0 && deltaY === 0) return;
		if (this.#applicationMousePressOwned(event.shiftKey)) {
			if (this.#reportApplicationWheel(event, deltaX, deltaY)) event.preventDefault();
			return;
		}
		if (deltaY === 0) return;
		const rows = this.#consumeLocalWheelRows(event.deltaMode, deltaY);
		if (rows === null) return;
		event.preventDefault();
		if (rows === 0) return;
		if (this.#store.metadata?.activeScreen === ActiveScreen.alternate) {
			if (!this.#inputEnabled) return;
			const key = rows < 0 ? 'ArrowUp' : 'ArrowDown';
			for (let index = 0; index < Math.abs(rows); index++) this.#sendSyntheticKey(key);
			return;
		}
		this.scrollLines(rows);
	};

	#reportApplicationWheel(event: WheelEvent, deltaX: number, deltaY: number): boolean {
		const metadata = this.#store.metadata;
		const renderer = this.#renderer;
		if (!metadata || !renderer) return false;
		const horizontal = consumeWheelSteps(
			this.#pendingApplicationWheelX,
			wheelDeltaPixels(deltaX, event.deltaMode, renderer.charWidth, metadata.columns),
			renderer.charWidth,
		);
		const vertical = consumeWheelSteps(
			this.#pendingApplicationWheelY,
			wheelDeltaPixels(deltaY, event.deltaMode, renderer.charHeight, metadata.rows),
			renderer.charHeight,
		);
		this.#pendingApplicationWheelX = horizontal.remainder;
		this.#pendingApplicationWheelY = vertical.remainder;
		const verticalButton = vertical.steps < 0 ? MouseButton.four : MouseButton.five;
		for (let index = 0; index < Math.abs(vertical.steps); index++) {
			this.#sendMouse(
				event,
				MouseAction.press,
				verticalButton,
				ProjectedInputPublicationIntent.stateless,
			);
		}
		const horizontalButton = horizontal.steps < 0 ? MouseButton.seven : MouseButton.six;
		for (let index = 0; index < Math.abs(horizontal.steps); index++) {
			this.#sendMouse(
				event,
				MouseAction.press,
				horizontalButton,
				ProjectedInputPublicationIntent.stateless,
			);
		}
		return true;
	}

	#consumeLocalWheelRows(deltaMode: number, deltaY: number): number | null {
		const metadata = this.#store.metadata;
		const renderer = this.#renderer;
		if (!metadata || !renderer) return null;
		const alternate = metadata.activeScreen === ActiveScreen.alternate;
		const pending = alternate ? this.#pendingAlternateWheelRows : this.#pendingHistoryWheelRows;
		const result = consumeWheelSteps(
			pending,
			wheelDeltaPixels(deltaY, deltaMode, renderer.charHeight, metadata.rows),
			renderer.charHeight,
		);
		if (alternate) this.#pendingAlternateWheelRows = result.remainder;
		else this.#pendingHistoryWheelRows = result.remainder;
		return result.steps;
	}

	#applicationMouseTrackingOwnsSurface(): boolean {
		if (!this.#inputEnabled) return false;
		const interaction = this.#store.metadata?.interaction;
		if (!interaction) return false;
		return (
			interaction.mouseAny ||
			interaction.mouseButton ||
			interaction.mouseNormal ||
			interaction.mouseX10
		);
	}

	#applicationMousePressOwned(shift: boolean): boolean {
		return !shift && this.#applicationMouseTrackingOwnsSurface();
	}

	#applicationMouseHoverOwned(shift: boolean): boolean {
		return !shift && this.#inputEnabled && this.#store.metadata?.interaction.mouseAny === true;
	}

	#mousePayload(
		event: MouseEvent,
		action: MouseInput['action'],
		button: MouseInput['button'],
		pressedButtons: number,
	): MouseInput | null {
		const metadata = this.#store.metadata;
		const canvas = this.#canvas;
		const renderer = this.#renderer;
		if (
			!metadata ||
			!canvas ||
			!renderer ||
			!Number.isFinite(event.clientX) ||
			!Number.isFinite(event.clientY)
		) {
			return null;
		}
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
		return {
			action,
			attachmentId: this.#attachmentId,
			button,
			modifiers: this.#modifiers(event),
			pressedButtons,
			x,
			y,
		};
	}

	#sendMouse(
		event: MouseEvent,
		action: MouseInput['action'],
		button: MouseInput['button'],
		intent: ProjectedInputPublicationIntent,
		onRejected?: () => void,
	): void {
		if (!this.#canAdmitInput()) return;
		const activeButton =
			action === MouseAction.motion && button === null
				? firstPressedMouseButton(mouseButtonSetMask(this.#pressedMouseButtons))
				: button;
		const inputOperationEpoch = this.#inputRepairEpoch;
		const previousInputState = this.#inputState;
		const activeButtonWasPressed =
			activeButton !== null && this.#pressedMouseButtons.has(activeButton);
		if (
			intent === ProjectedInputPublicationIntent.held &&
			action === MouseAction.press &&
			activeButton !== null
		) {
			this.#pressedMouseButtons.add(activeButton);
		} else if (
			intent === ProjectedInputPublicationIntent.held &&
			action === MouseAction.release &&
			activeButton !== null
		) {
			this.#pressedMouseButtons.delete(activeButton);
		}
		const payload = this.#mousePayload(
			event,
			action,
			activeButton,
			mouseButtonSetMask(this.#pressedMouseButtons),
		);
		if (payload === null) {
			if (activeButtonWasPressed) this.#pressedMouseButtons.add(activeButton);
			else if (activeButton !== null) this.#pressedMouseButtons.delete(activeButton);
			onRejected?.();
			return;
		}
		if (intent === ProjectedInputPublicationIntent.held) {
			this.#inputState = recordTerminalMouseSample(
				this.#inputState,
				this.#pressedMouseButtons.size === 0 ? undefined : payload,
			);
		}
		const stagedInputState = this.#inputState;
		this.#sendInput(
			{
				kind: 'mouse-input',
				payload,
			},
			intent,
			action === MouseAction.press && activeButton !== null
				? () => {
						if (!this.#inputOperationIsCurrent(inputOperationEpoch)) return;
						if (activeButtonWasPressed) this.#pressedMouseButtons.add(activeButton);
						else this.#pressedMouseButtons.delete(activeButton);
						if (this.#inputState === stagedInputState) {
							this.#inputState = previousInputState;
						}
						onRejected?.();
					}
				: undefined,
		);
	}

	#installResizeObserver(root: HTMLElement): void {
		if (typeof ResizeObserver === 'undefined') return;
		this.#resizeObserver = new ResizeObserver(() => this.#handleSurfaceResize());
		this.#resizeObserver.observe(root);
	}

	#handleSurfaceResize(): void {
		this.#invalidateLinkCache();
		this.#renderer?.syncSurfacePadding();
		this.#syncHistoryScrollbarPresentation();
		this.#updateInputPosition();
		this.#sendResizeIfChanged();
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
		this.#invalidateLinkCache();
		this.#syncDevicePixelRatio();
		this.#renderer?.syncSurfacePadding();
		this.#syncHistoryScrollbarPresentation();
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
		this.#invalidateLinkCache();
		this.#devicePixelRatio = Number.isFinite(ratio) && ratio > 0 ? ratio : 1;
		this.#watchDevicePixelRatio(view);
		this.#updateInputPosition();
		this.#sendResizeIfChanged();
	}

	#watchDevicePixelRatio(view: Window): void {
		this.#devicePixelRatioQuery?.removeEventListener('change', this.#handleDevicePixelRatioChange);
		this.#devicePixelRatioQuery = undefined;
		if (typeof view.matchMedia !== 'function') return;
		const query = view.matchMedia(`(resolution: ${this.#devicePixelRatio}dppx)`);
		query.addEventListener('change', this.#handleDevicePixelRatioChange);
		this.#devicePixelRatioQuery = query;
	}

	#applyInputPresentation(
		input: HTMLTextAreaElement,
		configuration: TerminalPresentationConfiguration,
	): void {
		input.style.fontFamily = configuration.fontFamily;
		input.style.fontSize = `${configuration.fontSize}px`;
		input.style.fontWeight = String(configuration.fontWeight);
		input.style.fontVariantLigatures = configuration.fontLigatures ? 'normal' : 'none';
	}

	#cancelPresentationPreparation(reason: 'disposed' | 'superseded'): void {
		this.#presentationPreparation?.controller.abort(reason);
		this.#presentationPreparation = undefined;
	}

	#commitPresentationConfiguration(
		renderer: WebGPURenderer,
		input: HTMLTextAreaElement,
		prepared: PreparedTerminalPresentationConfiguration,
	): void {
		this.#cancelScheduledFrame();
		this.#presentationConfiguration = prepared.configuration;
		let rendererError: unknown;
		try {
			renderer.commitPresentationConfiguration(prepared);
		} catch (error) {
			rendererError = error;
		}
		this.#applyInputPresentation(input, prepared.configuration);
		this.#refreshPreeditGeometry();
		this.#invalidateLinkCache();
		this.#finishHistoryScrollbarGesture(true);
		if (this.#selectionGestureActive || this.#selectionPointer !== undefined) {
			this.#cancelSelectionGesture(true);
		}
		this.#updateInputPosition();
		this.#sendResizeIfChanged();
		this.#scheduleRender();
		if (rendererError !== undefined) {
			this.#errorEmitter.fire({ error: rendererError, operation: 'render' });
		}
	}

	async #loadConfiguredFonts(
		document: Document,
		configuration: TerminalPresentationConfiguration,
	): Promise<void> {
		const fonts = document.fonts;
		if (!fonts || typeof fonts.load !== 'function') return;
		const descriptions = new Set([
			`${configuration.fontWeight} ${configuration.fontSize}px ${configuration.fontFamily}`,
			`italic ${configuration.fontWeight} ${configuration.fontSize}px ${configuration.fontFamily}`,
			`${configuration.fontWeightBold} ${configuration.fontSize}px ${configuration.fontFamily}`,
			`italic ${configuration.fontWeightBold} ${configuration.fontSize}px ${configuration.fontFamily}`,
		]);
		await Promise.all(
			[...descriptions].map((description) => fonts.load(description, TERMINAL_FONT_LOAD_SAMPLE)),
		);
	}

	#waitForPresentationCancellation(
		controller: AbortController,
	): Promise<PresentationFontPreparation> {
		return new Promise((resolve) => {
			controller.signal.addEventListener(
				'abort',
				() => {
					resolve({
						kind: 'cancelled',
						reason: controller.signal.reason === 'disposed' ? 'disposed' : 'superseded',
					});
				},
				{ once: true },
			);
		});
	}

	#sendResizeIfChanged(): void {
		if (
			this.#phase !== ProjectedTerminalPhase.open ||
			this.#terminalExited ||
			!this.#ownsResizeLease
		) {
			return;
		}
		const root = this.#root;
		const renderer = this.#renderer;
		if (!root || !renderer || root.clientWidth <= 0 || root.clientHeight <= 0) return;
		const metrics = renderer.getMetrics();
		const grid = calculateSurfaceGrid(
			{ height: root.clientHeight, width: root.clientWidth },
			{ height: metrics.height, width: metrics.width },
			readSurfacePadding(root),
		);
		if (grid === undefined) return;
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
			grid.columns,
			1,
			Math.min(
				PROTOCOL_LIMITS.maxFrameColumns,
				Math.floor(MAX_PTY_PIXEL_DIMENSION / cellPixelWidth),
			),
		);
		const rows = clamp(
			grid.rows,
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
		const committedMetadata = this.#store.metadata;
		if (committedMetadata !== null && sameGeometry(geometry, committedMetadata)) {
			this.#lastGeometry = undefined;
			return;
		}
		const now = projectedNow();
		if (
			sameGeometry(geometry, this.#lastGeometry) &&
			now - this.#lastGeometryAttemptedAt < RESIZE_REENTRY_MS
		) {
			return;
		}
		this.#lastGeometry = geometry;
		this.#lastGeometryAttemptedAt = now;
		this.#send({
			kind: 'resize-terminal',
			payload: { attachmentId: this.#attachmentId, ...geometry },
		});
	}

	#reconcileCommittedGeometry(metadata: ProjectedTerminalMetadata): void {
		if (this.#lastGeometry !== undefined && !sameGeometry(this.#lastGeometry, metadata)) {
			this.#lastGeometry = undefined;
		}
	}

	#updateAccessibility(metadata: ProjectedTerminalMetadata): void {
		const root = this.#accessibilityRoot;
		if (!root) return;
		this.#accessibilityAnnouncement?.setAttribute(
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
			this.#accessibilityModelRows.pop();
		}
		for (let rowIndex = 0; rowIndex < metadata.rows; rowIndex++) {
			const row = store.rows[rowIndex];
			const element = this.#accessibilityRows[rowIndex];
			if (!row || !element) continue;
			if (row.dirty || store.needsFullRedraw || !this.#accessibilityModelRows[rowIndex]) {
				const cells = Array.from({ length: metadata.columns }, (_, column) => {
					const width = projectedCellDisplayWidth(row.widths[column] ?? 0);
					return { text: width > 0 ? row.grapheme(column) : '', width };
				});
				const accessibleRow = buildAccessibleRow(cells, metadata.columns);
				element.textContent = accessibleRow.text;
				this.#accessibilityModelRows[rowIndex] = accessibleRow;
			}
			const absoluteRow =
				this.#historyPhase === ProjectedHistoryPhase.followingBottom
					? this.#historyTotalRows - BigInt(metadata.rows) + BigInt(rowIndex)
					: this.#historyStartRow + BigInt(rowIndex);
			const rowLabel = this.#options.labels.row;
			if (rowLabel !== undefined) {
				element.setAttribute(
					'aria-label',
					rowLabel(absoluteRow + 1n, this.#accessibilityModelRows[rowIndex]?.text ?? ' '),
				);
			}
			element.setAttribute('aria-posinset', (absoluteRow + 1n).toString());
			element.setAttribute('aria-setsize', this.#historyTotalRows.toString());
			element.setAttribute('data-terminal-absolute-row', absoluteRow.toString());
		}
		this.#accessibilityModel.update(this.#accessibilityModelRows);
		const visibleStart =
			this.#historyPhase === ProjectedHistoryPhase.followingBottom
				? this.#historyTotalRows - BigInt(metadata.rows)
				: this.#historyStartRow;
		this.#accessibilityVisibleStart = visibleStart;
		const visibleEnd =
			this.#historyTotalRows === 0n
				? 0n
				: minBigInt(this.#historyTotalRows, visibleStart + BigInt(metadata.rows));
		const screenCursor = this.#options.labels.screenCursor;
		root.setAttribute(
			'aria-label',
			this.#historyPhase === ProjectedHistoryPhase.followingBottom && screenCursor !== undefined
				? screenCursor(visibleStart + BigInt(metadata.cursor.row) + 1n, metadata.cursor.column + 1)
				: this.#options.labels.screen,
		);
		this.#updateAccessibilityDescription(metadata, visibleStart, visibleEnd);
		root.setAttribute(
			'data-terminal-visible-row-start',
			(this.#historyPhase === ProjectedHistoryPhase.followingBottom
				? this.#historyTotalRows - BigInt(metadata.rows)
				: this.#historyStartRow
			).toString(),
		);
		root.setAttribute('data-terminal-retained-rows', this.#historyTotalRows.toString());
		this.#updateAccessibilityAnnouncement(metadata);
		this.#syncAccessibilitySelection();
	}

	#updateAccessibilityDescription(
		metadata = this.metadata,
		visibleStart?: bigint,
		visibleEnd?: bigint,
	): void {
		const root = this.#accessibilityRoot;
		if (root === undefined || metadata === null) return;
		const start =
			visibleStart ??
			(this.#historyPhase === ProjectedHistoryPhase.followingBottom
				? this.#historyTotalRows - BigInt(metadata.rows)
				: this.#historyStartRow);
		const end =
			visibleEnd ??
			(this.#historyTotalRows === 0n
				? 0n
				: minBigInt(this.#historyTotalRows, start + BigInt(metadata.rows)));
		const descriptions: string[] = [];
		const historyRange = this.#options.labels.historyRange;
		if (historyRange !== undefined) {
			descriptions.push(
				historyRange(this.#historyTotalRows === 0n ? 0n : start + 1n, end, this.#historyTotalRows),
			);
		}
		if (this.#selection.state === 'active' && this.#selection.preview) {
			const selectedText = this.#options.labels.selectedText;
			if (selectedText !== undefined) descriptions.push(selectedText(this.#selection.preview));
		}
		root.setAttribute('aria-description', descriptions.join(' '));
	}

	#updateAccessibilityAnnouncement(metadata: ProjectedTerminalMetadata): void {
		if (this.#historyPhase !== ProjectedHistoryPhase.followingBottom) return;
		const rows = this.#accessibilityModelRows.map((row) => row.text);
		const cursor = metadata.cursor;
		const previousCursor = this.#accessibilityPreviousCursor;
		const deletion =
			previousCursor?.row === cursor.row
				? detectAccessibilityDeletion(
						this.#accessibilityPreviousModelRows[cursor.row] ?? buildAccessibleRow([], 0),
						this.#accessibilityModelRows[cursor.row] ?? buildAccessibleRow([], 0),
						previousCursor.column,
						cursor.column,
					)
				: null;
		const changedRows = createAccessibilityAnnouncement(rows, this.#accessibilityPreviousRows);
		const announcement =
			deletion !== null && this.#options.labels.deleted !== undefined
				? this.#options.labels.deleted(deletion)
				: changedRows;
		const liveRegion = this.#accessibilityAnnouncement;
		if (announcement && liveRegion !== undefined && liveRegion.textContent !== announcement) {
			liveRegion.textContent = announcement;
		}
		this.#accessibilityPreviousRows = rows;
		this.#accessibilityPreviousModelRows = [...this.#accessibilityModelRows];
		this.#accessibilityPreviousCursor = { column: cursor.column, row: cursor.row };
	}

	readonly #handleAccessibilitySelectionChange = (): void => {
		if (
			this.#phase === ProjectedTerminalPhase.disposed ||
			this.#synchronizingAccessibilitySelection ||
			this.#selectionGestureInFlight ||
			this.#selectionGestureActive
		) {
			return;
		}
		const selection = this.#root?.ownerDocument.getSelection();
		if (selection === undefined || selection === null || selection.rangeCount !== 1) {
			this.#relinquishAccessibilitySelectionOwnership();
			return;
		}
		const range = selection.getRangeAt(0);
		const startOffset = this.#accessibilitySelectionOffset(range.startContainer, range.startOffset);
		const endOffset = this.#accessibilitySelectionOffset(range.endContainer, range.endOffset);
		if (startOffset === null || endOffset === null) {
			this.#relinquishAccessibilitySelectionOwnership();
			return;
		}
		if (selection.isCollapsed || startOffset === endOffset) {
			this.#clearAccessibilitySelectionFromDom();
			return;
		}
		const normalizedOffsets = {
			end: Math.max(startOffset, endOffset),
			start: Math.min(startOffset, endOffset),
		};
		if (
			this.#accessibilityAppliedOffsets?.start === normalizedOffsets.start &&
			this.#accessibilityAppliedOffsets.end === normalizedOffsets.end
		) {
			this.#accessibilityAppliedOffsets = undefined;
			this.#accessibilityOwnsDomSelection = true;
			return;
		}
		const terminalRange = this.#accessibilityModel.offsetsToInclusiveTerminalRange({
			end: normalizedOffsets.end,
			start: normalizedOffsets.start,
		});
		const start = this.#accessibilitySelectionPoint(terminalRange.start);
		const end = this.#accessibilitySelectionPoint(terminalRange.end);
		if (start === null || end === null) return;
		this.#accessibilityOwnsDomSelection = true;
		this.#sendSelectionRange(start, end);
	};

	#clearAccessibilitySelectionFromDom(): void {
		if (!this.#accessibilityOwnsDomSelection) return;
		this.#relinquishAccessibilitySelectionOwnership();
		this.#sendSelectionCommand('clear');
	}

	#relinquishAccessibilitySelectionOwnership(): void {
		this.#accessibilityAppliedOffsets = undefined;
		this.#accessibilityOwnsDomSelection = false;
	}

	#accessibilitySelectionOffset(node: Node, offset: number): number | null {
		const element = isElementNode(node) ? node : node.parentElement;
		if (element === null) return null;
		const rowElement = element.closest('[data-terminal-absolute-row]');
		if (rowElement === null || !this.#accessibilityRoot?.contains(rowElement)) return null;
		const rowIndex = this.#accessibilityRows.indexOf(rowElement);
		if (rowIndex < 0) return null;
		const rowStart = this.#accessibilityModel.positionToOffset({ column: 0, row: rowIndex });
		const rowLength = this.#accessibilityModelRows[rowIndex]?.text.length ?? 0;
		const localOffset = node === rowElement ? (offset > 0 ? rowLength : 0) : offset;
		return rowStart + clamp(localOffset, 0, rowLength);
	}

	#accessibilitySelectionPoint(position: {
		readonly column: number;
		readonly row: number;
	}): SelectionPoint | null {
		const rowElement = this.#accessibilityRows[position.row];
		const absoluteRow = rowElement?.getAttribute('data-terminal-absolute-row');
		if (absoluteRow === undefined || absoluteRow === null) return null;
		return {
			column: position.column,
			row: CanonicalUint64TextSchema.parse(absoluteRow),
		};
	}

	#syncAccessibilitySelection(): void {
		const root = this.#accessibilityRoot;
		const selection = this.#selection;
		if (
			root === undefined ||
			selection.state !== 'active' ||
			selection.rectangle ||
			typeof root.ownerDocument.createRange !== 'function'
		) {
			this.#clearOwnedAccessibilityDomSelection();
			return;
		}

		const startRow = BigInt(selection.start.row) - this.#accessibilityVisibleStart;
		const endRow = BigInt(selection.end.row) - this.#accessibilityVisibleStart;
		if (
			startRow < 0n ||
			endRow < 0n ||
			startRow >= BigInt(this.#accessibilityModelRows.length) ||
			endRow >= BigInt(this.#accessibilityModelRows.length)
		) {
			this.#clearOwnedAccessibilityDomSelection();
			return;
		}

		const offsets = this.#accessibilityModel.inclusiveTerminalRangeToOffsets({
			end: { column: selection.end.column, row: Number(endRow) },
			start: { column: selection.start.column, row: Number(startRow) },
		});
		const start = this.#accessibilityDomPoint(
			this.#accessibilityModel.offsetToPosition(offsets.start, 'backward'),
		);
		const end = this.#accessibilityDomPoint(
			this.#accessibilityModel.offsetToPosition(offsets.end, 'forward'),
		);
		const domSelection = root.ownerDocument.getSelection();
		if (start === null || end === null || domSelection === null) return;

		const range = root.ownerDocument.createRange();
		try {
			range.setStart(start.node, start.offset);
			range.setEnd(end.node, end.offset);
		} catch {
			return;
		}
		this.#synchronizingAccessibilitySelection = true;
		try {
			domSelection.removeAllRanges();
			domSelection.addRange(range);
			this.#accessibilityAppliedOffsets = offsets;
			this.#accessibilityOwnsDomSelection = true;
		} finally {
			this.#synchronizingAccessibilitySelection = false;
		}
	}

	#accessibilityDomPoint(position: {
		readonly column: number;
		readonly row: number;
	}): { readonly node: Node; readonly offset: number } | null {
		const row = this.#accessibilityRows[position.row];
		const node = row?.firstChild;
		if (!row || !node) return null;
		const rowStart = this.#accessibilityModel.positionToOffset({
			column: 0,
			row: position.row,
		});
		return {
			node,
			offset: clamp(
				this.#accessibilityModel.positionToOffset(position) - rowStart,
				0,
				row.textContent?.length ?? 0,
			),
		};
	}

	#clearOwnedAccessibilityDomSelection(): void {
		const root = this.#accessibilityRoot;
		const selection = root?.ownerDocument.getSelection();
		const ownsCurrentSelection =
			selection?.anchorNode !== null &&
			selection?.anchorNode !== undefined &&
			root?.contains(selection.anchorNode);
		if (!this.#accessibilityOwnsDomSelection && !ownsCurrentSelection) return;
		this.#relinquishAccessibilitySelectionOwnership();
		this.#synchronizingAccessibilitySelection = true;
		try {
			selection?.removeAllRanges();
		} finally {
			this.#synchronizingAccessibilitySelection = false;
		}
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

function normalizedKeyboardCode(code: string): string {
	return LEGACY_KEY_CODE_ALIASES.get(code) ?? code;
}

function keyboardKeyText(key: string): string {
	const firstCodepoint = key.codePointAt(0);
	if (
		firstCodepoint === undefined ||
		(firstCodepoint >= 0xd800 && firstCodepoint <= 0xdfff) ||
		!W3C_KEY_STRING_PATTERN.test(key)
	) {
		return '';
	}
	return key;
}

function terminalKeyCode(code: string, text: string): TerminalKeyCodeValue | null {
	return terminalKeyCodes.get(code) ?? (text ? 'Unidentified' : null);
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

function mouseButtonSetMask(buttons: Iterable<Exclude<MouseInput['button'], null>>): number {
	let mask = 0;
	for (const button of buttons) mask |= 1 << (button - 1);
	return mask;
}

function mouseReleasePayload(
	sample: MouseInput | undefined,
	button: Exclude<MouseInput['button'], null>,
	pressedButtons: number,
): MouseInput | null {
	return sample === undefined
		? null
		: { ...sample, action: MouseAction.release, button, pressedButtons };
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

function wheelDeltaPixels(
	delta: number,
	deltaMode: number,
	cellPixels: number,
	pageCells: number,
): number {
	if (!Number.isFinite(delta)) return 0;
	const cell = Number.isFinite(cellPixels) ? Math.max(1, cellPixels) : 1;
	let multiplier = 1;
	if (deltaMode === 1) multiplier = cell;
	else if (deltaMode === 2) multiplier = cell * Math.max(1, pageCells);
	const pixels = delta * multiplier;
	return Number.isFinite(pixels) ? pixels : 0;
}

function consumeWheelSteps(
	pending: number,
	deltaPixels: number,
	stepPixels: number,
): { readonly remainder: number; readonly steps: number } {
	const step = Number.isFinite(stepPixels) ? Math.max(1, stepPixels) : 1;
	const total = pending + deltaPixels;
	if (!Number.isFinite(total)) return { remainder: 0, steps: 0 };
	const steps = Math.trunc(total / step);
	return { remainder: total - steps * step, steps };
}

function searchAnchorFromSelection(
	selection: SelectionChanged,
): { readonly column: number; readonly row: number } | undefined {
	if (selection.state !== 'active') return undefined;
	const row = BigInt(selection.start.row);
	if (row > BigInt(Number.MAX_SAFE_INTEGER)) return undefined;
	return { column: selection.start.column, row: Number(row) };
}

function selectionMatchesSearch(
	selection: SelectionChanged,
	match: TerminalSearchMatch | null,
): boolean {
	if (selection.state !== 'active' || match === null || selection.rectangle) return false;
	return (
		selection.start.column === match.start.column &&
		BigInt(selection.start.row) === BigInt(match.start.row) &&
		selection.end.column === match.end.column &&
		BigInt(selection.end.row) === BigInt(match.end.row)
	);
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
