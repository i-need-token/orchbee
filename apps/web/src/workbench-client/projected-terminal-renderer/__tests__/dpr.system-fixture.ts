import {
	ProjectedTerminal,
	type ProjectedTerminalCommand,
	type ProjectedTerminalFrameTelemetry,
} from '@orchbee/terminal';
import {
	ActiveScreen,
	AttachmentIdSchema,
	CellWidth,
	CreationIdSchema,
	CursorShape,
	encodeTerminalFrame,
	HostEpochSchema,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalFrameKind,
	TerminalIdSchema,
	UnderlineKind,
	type TerminalFrame,
} from '@orchbee/terminal-protocol';

const attachmentId = AttachmentIdSchema.parse('22'.repeat(16));
const creationId = CreationIdSchema.parse('33'.repeat(16));
const hostEpoch = HostEpochSchema.parse('11'.repeat(16));
const terminalId = TerminalIdSchema.parse('44'.repeat(16));
const columns = 24;
const rows = 4;
const cursor = { column: 3, row: 2 } as const;

const root = document.createElement('main');
root.id = 'terminal-dpr-witness';
root.dataset['witnessAtlasClearCount'] = '0';
root.dataset['witnessCanvasFillTextCount'] = '0';
root.dataset['witnessMatchMediaActiveCount'] = '0';
root.dataset['witnessMatchMediaAddCount'] = '0';
root.dataset['witnessMatchMediaCallCount'] = '0';
root.dataset['witnessMatchMediaInvocationCount'] = '0';
root.dataset['witnessMatchMediaRemoveCount'] = '0';
root.dataset['witnessRenderCount'] = '0';
root.dataset['witnessResizeCount'] = '0';
root.dataset['witnessTerminalMutationCount'] = '0';
root.dataset['witnessWindowResizeActiveCount'] = '0';
root.dataset['witnessWindowResizeInvocationCount'] = '0';
root.dataset['witnessControl'] = 'cdp-device-metrics-with-browser-resize';
root.dataset['witnessLimitation'] = 'synthetic-chromium-dpr-transition-not-native-monitor-transfer';
root.style.boxSizing = 'border-box';
root.style.height = '240px';
root.style.overflow = 'hidden';
root.style.position = 'relative';
root.style.width = '720px';

const disposeButton = document.createElement('button');
disposeButton.id = 'dispose-dpr-witness';
disposeButton.textContent = 'Dispose DPR witness';
document.body.replaceChildren(root, disposeButton);
document.body.style.margin = '0';

installCanvasInstrumentation();
installBrowserListenerInstrumentation();
forceCanvasRendererForWitness();

const commands: ProjectedTerminalCommand[] = [];
let initial: InitialObservation | undefined;
let lastResize: Extract<ProjectedTerminalCommand, { readonly kind: 'resize-terminal' }> | undefined;

const terminal = new ProjectedTerminal({
	attachmentId,
	canResize: true,
	creationId,
	hostEpoch,
	labels: { input: 'DPR witness input', screen: 'DPR witness screen' },
	port: {
		send: (command) => {
			commands.push(command);
			if (command.kind !== 'resize-terminal') return;
			lastResize = command;
			incrementDataset(root, 'witnessResizeCount');
			incrementDataset(root, 'witnessTerminalMutationCount');
			root.dataset['witnessResizeCellPixelHeight'] = String(command.payload.cellPixelHeight);
			root.dataset['witnessResizeCellPixelWidth'] = String(command.payload.cellPixelWidth);
			root.dataset['witnessResizeColumns'] = String(command.payload.columns);
			root.dataset['witnessResizeRows'] = String(command.payload.rows);
			root.dataset['witnessResizeTerminalPixelHeight'] = String(
				command.payload.cellPixelHeight * command.payload.rows,
			);
			root.dataset['witnessResizeTerminalPixelWidth'] = String(
				command.payload.cellPixelWidth * command.payload.columns,
			);
		},
	},
	terminalId,
});

terminal.onFrameTelemetry((event) => {
	incrementDataset(root, 'witnessRenderCount');
	incrementDataset(root, 'witnessTerminalMutationCount');
	root.dataset['witnessRenderer'] = event.backend;
	root.dataset['witnessLastFrameId'] = String(event.frameId);
	root.dataset['witnessLastGeneration'] = event.generation;
	root.dataset['witnessLastRevision'] = event.revision;
	root.dataset['witnessLastReasons'] = event.reasons.join(',');
	root.dataset['witnessLastForceAll'] = String(event.forceAll);
	root.dataset['witnessLastRowsRendered'] = String(event.rowsRendered);
	root.dataset['witnessLastCellsVisited'] = String(event.cellsVisited);
	tryFinalizeTransition(event);
});

disposeButton.addEventListener('click', () => {
	try {
		terminal.dispose();
		root.dataset['witnessDisposed'] = 'true';
		root.dataset['witnessDisposeChildCount'] = String(root.childElementCount);
		root.dataset['witnessMutationCountAtDispose'] =
			root.dataset['witnessTerminalMutationCount'] ?? '0';
		root.dataset['witnessRenderCountAtDispose'] = root.dataset['witnessRenderCount'] ?? '0';
		root.dataset['witnessResizeCountAtDispose'] = root.dataset['witnessResizeCount'] ?? '0';
		if (Number(root.dataset['witnessMatchMediaActiveCount'] ?? '-1') !== 0) {
			throw new Error('Disposal retained a matchMedia listener.');
		}
		if (Number(root.dataset['witnessWindowResizeActiveCount'] ?? '-1') !== 0) {
			throw new Error('Disposal retained a window resize listener.');
		}
		if (root.childElementCount !== 0) {
			throw new Error('Disposal retained terminal-owned DOM.');
		}
		root.dataset['witnessDisposeComplete'] = 'true';
	} catch (error) {
		reportFailure(error);
	}
});

void startWitness();

interface InitialObservation {
	readonly atlasClearCount: number;
	readonly canvasCssHeight: number;
	readonly canvasCssWidth: number;
	readonly canvasFillTextCount: number;
	readonly canvasHeight: number;
	readonly canvasWidth: number;
	readonly devicePixelRatio: number;
	readonly inputGeometry: string;
	readonly renderCount: number;
	readonly resizeColumns: number;
	readonly resizeCount: number;
	readonly resizeRows: number;
}

async function startWitness(): Promise<void> {
	try {
		await terminal.open(root);
		if (terminal.applyFrame(terminalSnapshot()) !== 'committed') {
			throw new Error('DPR witness frame did not commit.');
		}
		await nextPaint();
		const canvas = terminal.canvas;
		const input = terminal.textarea;
		const metadata = terminal.metadata;
		const resize = lastResize;
		if (canvas === undefined || input === undefined || metadata === null || resize === undefined) {
			throw new Error('DPR witness surface is incomplete.');
		}
		if (metadata.columns !== columns || metadata.rows !== rows) {
			throw new Error('DPR witness committed unexpected logical geometry.');
		}
		const canvasCssWidth = cssPixelNumber(canvas.style.width);
		const canvasCssHeight = cssPixelNumber(canvas.style.height);
		initial = {
			atlasClearCount: datasetNumber('witnessAtlasClearCount'),
			canvasCssHeight,
			canvasCssWidth,
			canvasFillTextCount: datasetNumber('witnessCanvasFillTextCount'),
			canvasHeight: canvas.height,
			canvasWidth: canvas.width,
			devicePixelRatio: window.devicePixelRatio,
			inputGeometry: inputGeometry(input),
			renderCount: datasetNumber('witnessRenderCount'),
			resizeColumns: resize.payload.columns,
			resizeCount: datasetNumber('witnessResizeCount'),
			resizeRows: resize.payload.rows,
		};
		root.dataset['witnessInitialDpr'] = String(initial.devicePixelRatio);
		root.dataset['witnessInitialColumns'] = String(metadata.columns);
		root.dataset['witnessInitialRows'] = String(metadata.rows);
		root.dataset['witnessInitialCanvasCssHeight'] = String(canvasCssHeight);
		root.dataset['witnessInitialCanvasCssWidth'] = String(canvasCssWidth);
		root.dataset['witnessInitialCanvasHeight'] = String(canvas.height);
		root.dataset['witnessInitialCanvasWidth'] = String(canvas.width);
		root.dataset['witnessInitialInputGeometry'] = initial.inputGeometry;
		root.dataset['witnessInitialAtlasClearCount'] = String(initial.atlasClearCount);
		root.dataset['witnessInitialCanvasFillTextCount'] = String(initial.canvasFillTextCount);
		root.dataset['witnessInitialRenderCount'] = String(initial.renderCount);
		root.dataset['witnessInitialResizeCount'] = String(initial.resizeCount);
		root.dataset['witnessReady'] = 'true';
	} catch (error) {
		reportFailure(error);
	}
}

function tryFinalizeTransition(event: ProjectedTerminalFrameTelemetry): void {
	const baseline = initial;
	if (baseline === undefined || window.devicePixelRatio === baseline.devicePixelRatio) return;
	try {
		const canvas = terminal.canvas;
		const input = terminal.textarea;
		const metadata = terminal.metadata;
		const resize = lastResize;
		if (canvas === undefined || input === undefined || metadata === null || resize === undefined) {
			throw new Error('DPR transition lost its current surface or resize.');
		}
		const canvasCssWidth = cssPixelNumber(canvas.style.width);
		const canvasCssHeight = cssPixelNumber(canvas.style.height);
		const ratio = window.devicePixelRatio;
		const cellCssWidth = canvasCssWidth / metadata.columns;
		const cellCssHeight = canvasCssHeight / metadata.rows;
		const expectedInput = expectedInputGeometry(canvas, cellCssWidth, cellCssHeight);
		const currentInput = inputGeometry(input);
		const sample = canvas
			.getContext('2d')
			?.getImageData(Math.max(0, canvas.width - 1), Math.max(0, canvas.height - 1), 1, 1).data;

		if (metadata.columns !== columns || metadata.rows !== rows) {
			throw new Error('DPR transition changed logical terminal geometry.');
		}
		if (
			canvasCssWidth !== baseline.canvasCssWidth ||
			canvasCssHeight !== baseline.canvasCssHeight
		) {
			throw new Error('DPR transition changed CSS terminal geometry.');
		}
		if (canvas.width !== canvasCssWidth * ratio || canvas.height !== canvasCssHeight * ratio) {
			throw new Error('DPR transition did not rebuild the Canvas backing dimensions.');
		}
		if (datasetNumber('witnessAtlasClearCount') !== baseline.atlasClearCount + 1) {
			throw new Error('DPR transition did not invalidate the raster atlas exactly once.');
		}
		if (datasetNumber('witnessCanvasFillTextCount') <= baseline.canvasFillTextCount) {
			throw new Error('DPR transition did not rerasterize the current Canvas text.');
		}
		if (
			event.backend !== 'canvas2d' ||
			!event.forceAll ||
			!event.reasons.includes('surface') ||
			event.rowsRendered !== rows ||
			event.cellsVisited !== rows * columns ||
			event.generation !== '1' ||
			event.revision !== '1'
		) {
			throw new Error('DPR transition did not complete one full current Canvas repaint.');
		}
		if (datasetNumber('witnessResizeCount') !== baseline.resizeCount + 1) {
			throw new Error('DPR transition did not emit exactly one current resize.');
		}
		if (
			resize.payload.columns !== baseline.resizeColumns ||
			resize.payload.rows !== baseline.resizeRows
		) {
			throw new Error('DPR transition changed the fitted PTY row or column count.');
		}
		if (
			resize.payload.cellPixelWidth !== Math.round(cellCssWidth * ratio) ||
			resize.payload.cellPixelHeight !== Math.round(cellCssHeight * ratio)
		) {
			throw new Error('DPR resize cell pixels do not match current renderer geometry.');
		}
		if (currentInput !== expectedInput || currentInput !== baseline.inputGeometry) {
			throw new Error('DPR transition left the hidden input anchor incoherent.');
		}
		if (
			datasetNumber('witnessMatchMediaCallCount') !== 2 ||
			datasetNumber('witnessMatchMediaActiveCount') !== 1 ||
			datasetNumber('witnessMatchMediaRemoveCount') !== 1 ||
			root.dataset['witnessMatchMediaLastQuery'] !== `(resolution: ${ratio}dppx)`
		) {
			throw new Error('DPR transition did not re-arm the current matchMedia listener.');
		}
		if (datasetNumber('witnessWindowResizeInvocationCount') < 1) {
			throw new Error('DPR transition did not enter through the browser resize listener.');
		}

		root.dataset['witnessCurrentDpr'] = String(ratio);
		root.dataset['witnessCurrentColumns'] = String(metadata.columns);
		root.dataset['witnessCurrentRows'] = String(metadata.rows);
		root.dataset['witnessCurrentCanvasCssHeight'] = String(canvasCssHeight);
		root.dataset['witnessCurrentCanvasCssWidth'] = String(canvasCssWidth);
		root.dataset['witnessCurrentCanvasHeight'] = String(canvas.height);
		root.dataset['witnessCurrentCanvasWidth'] = String(canvas.width);
		root.dataset['witnessCurrentInputGeometry'] = currentInput;
		root.dataset['witnessCurrentCanvasAlpha'] = String(sample?.[3] ?? -1);
		root.dataset['witnessAtlasClearDelta'] = String(
			datasetNumber('witnessAtlasClearCount') - baseline.atlasClearCount,
		);
		root.dataset['witnessCanvasFillTextDelta'] = String(
			datasetNumber('witnessCanvasFillTextCount') - baseline.canvasFillTextCount,
		);
		root.dataset['witnessRenderDelta'] = String(
			datasetNumber('witnessRenderCount') - baseline.renderCount,
		);
		root.dataset['witnessResizeDelta'] = String(
			datasetNumber('witnessResizeCount') - baseline.resizeCount,
		);
		root.dataset['witnessInputAnchorCoherent'] = 'true';
		root.dataset['witnessDprTransition'] = 'complete';
	} catch (error) {
		reportFailure(error);
	}
}

function installCanvasInstrumentation(): void {
	const clearRect = CanvasRenderingContext2D.prototype.clearRect;
	const fillText = CanvasRenderingContext2D.prototype.fillText;
	Object.defineProperty(CanvasRenderingContext2D.prototype, 'clearRect', {
		configurable: true,
		value: function (
			this: CanvasRenderingContext2D,
			x: number,
			y: number,
			width: number,
			height: number,
		) {
			if (
				this.canvas.width === 2_048 &&
				this.canvas.height === 2_048 &&
				x === 0 &&
				y === 0 &&
				width === 2_048 &&
				height === 2_048
			) {
				incrementDataset(root, 'witnessAtlasClearCount');
			}
			clearRect.call(this, x, y, width, height);
		},
	});
	Object.defineProperty(CanvasRenderingContext2D.prototype, 'fillText', {
		configurable: true,
		value: function (
			this: CanvasRenderingContext2D,
			text: string,
			x: number,
			y: number,
			maximumWidth?: number,
		) {
			if (this.canvas.parentElement === root) {
				incrementDataset(root, 'witnessCanvasFillTextCount');
			}
			if (maximumWidth === undefined) fillText.call(this, text, x, y);
			else fillText.call(this, text, x, y, maximumWidth);
		},
	});
}

function installBrowserListenerInstrumentation(): void {
	type BrowserListener = EventListenerOrEventListenerObject;
	type Add = (
		type: string,
		listener: BrowserListener,
		options?: boolean | AddEventListenerOptions,
	) => void;
	type Remove = (
		type: string,
		listener: BrowserListener,
		options?: boolean | EventListenerOptions,
	) => void;
	const nativeWindowAdd: Add = window.addEventListener.bind(window);
	const nativeWindowRemove: Remove = window.removeEventListener.bind(window);
	const resizeWrappers = new Map<BrowserListener, EventListener>();
	Object.defineProperty(window, 'addEventListener', {
		configurable: true,
		value: ((
			type: string,
			listener: BrowserListener,
			options?: boolean | AddEventListenerOptions,
		) => {
			if (type !== 'resize') {
				nativeWindowAdd(type, listener, options);
				return;
			}
			const wrapped: EventListener = (event) => {
				incrementDataset(root, 'witnessWindowResizeInvocationCount');
				invokeListener(listener, window, event);
			};
			resizeWrappers.set(listener, wrapped);
			root.dataset['witnessWindowResizeActiveCount'] = String(resizeWrappers.size);
			nativeWindowAdd(type, wrapped, options);
		}) satisfies Add,
	});
	Object.defineProperty(window, 'removeEventListener', {
		configurable: true,
		value: ((type: string, listener: BrowserListener, options?: boolean | EventListenerOptions) => {
			const wrapped = type === 'resize' ? resizeWrappers.get(listener) : undefined;
			if (wrapped === undefined) {
				nativeWindowRemove(type, listener, options);
				return;
			}
			resizeWrappers.delete(listener);
			root.dataset['witnessWindowResizeActiveCount'] = String(resizeWrappers.size);
			nativeWindowRemove(type, wrapped, options);
		}) satisfies Remove,
	});

	const nativeMatchMedia = window.matchMedia.bind(window);
	Object.defineProperty(window, 'matchMedia', {
		configurable: true,
		value: (media: string): MediaQueryList => {
			incrementDataset(root, 'witnessMatchMediaCallCount');
			root.dataset['witnessMatchMediaLastQuery'] = media;
			const query = nativeMatchMedia(media);
			const nativeAdd: Add = query.addEventListener.bind(query);
			const nativeRemove: Remove = query.removeEventListener.bind(query);
			const wrappers = new Map<BrowserListener, EventListener>();
			Object.defineProperty(query, 'addEventListener', {
				configurable: true,
				value: ((
					type: string,
					listener: BrowserListener,
					options?: boolean | AddEventListenerOptions,
				) => {
					if (type !== 'change') {
						nativeAdd(type, listener, options);
						return;
					}
					const wrapped: EventListener = (event) => {
						incrementDataset(root, 'witnessMatchMediaInvocationCount');
						invokeListener(listener, query, event);
					};
					wrappers.set(listener, wrapped);
					incrementDataset(root, 'witnessMatchMediaAddCount');
					root.dataset['witnessMatchMediaActiveCount'] = String(
						datasetNumber('witnessMatchMediaActiveCount') + 1,
					);
					nativeAdd(type, wrapped, options);
				}) satisfies Add,
			});
			Object.defineProperty(query, 'removeEventListener', {
				configurable: true,
				value: ((
					type: string,
					listener: BrowserListener,
					options?: boolean | EventListenerOptions,
				) => {
					const wrapped = type === 'change' ? wrappers.get(listener) : undefined;
					if (wrapped === undefined) {
						nativeRemove(type, listener, options);
						return;
					}
					wrappers.delete(listener);
					incrementDataset(root, 'witnessMatchMediaRemoveCount');
					root.dataset['witnessMatchMediaActiveCount'] = String(
						datasetNumber('witnessMatchMediaActiveCount') - 1,
					);
					nativeRemove(type, wrapped, options);
				}) satisfies Remove,
			});
			return query;
		},
	});
}

function invokeListener(
	listener: EventListenerOrEventListenerObject,
	target: EventTarget,
	event: Event,
): void {
	if (typeof listener === 'function') listener.call(target, event);
	else listener.handleEvent(event);
}

function forceCanvasRendererForWitness(): void {
	Object.defineProperty(navigator, 'gpu', {
		configurable: true,
		value: undefined,
	});
	root.dataset['witnessForcedCanvas'] = 'true';
}

function expectedInputGeometry(
	canvas: HTMLCanvasElement,
	cellWidth: number,
	cellHeight: number,
): string {
	const rootBounds = root.getBoundingClientRect();
	const canvasBounds = canvas.getBoundingClientRect();
	return [
		canvasBounds.left - rootBounds.left + (cursor.column + 0.5) * cellWidth,
		canvasBounds.top - rootBounds.top + cursor.row * cellHeight,
		cellWidth,
		cellHeight,
		cellHeight,
	]
		.map(cssGeometryNumber)
		.join(',');
}

function inputGeometry(input: HTMLTextAreaElement): string {
	return [
		input.style.left,
		input.style.top,
		input.style.width,
		input.style.height,
		input.style.lineHeight,
	]
		.map(cssPixelNumber)
		.map(cssGeometryNumber)
		.join(',');
}

function cssPixelNumber(value: string): number {
	const parsed = Number.parseFloat(value);
	if (!Number.isFinite(parsed)) throw new Error(`Invalid CSS pixel value: ${value}`);
	return parsed;
}

function cssGeometryNumber(value: number): number {
	return Math.round(value * 1_000) / 1_000;
}

function datasetNumber(key: string): number {
	return Number(root.dataset[key] ?? '0');
}

function incrementDataset(element: HTMLElement, key: string): void {
	element.dataset[key] = String(Number(element.dataset[key] ?? '0') + 1);
}

function reportFailure(error: unknown): void {
	root.dataset['witnessFailure'] = error instanceof Error ? error.message : String(error);
}

async function nextPaint(): Promise<void> {
	await new Promise<void>((resolve) => {
		requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
	});
}

function terminalSnapshot(): Uint8Array {
	const style: TerminalFrame['styles'][number] = {
		background: { kind: TerminalColorKind.default },
		blink: false,
		bold: false,
		faint: false,
		foreground: { kind: TerminalColorKind.default },
		hyperlink: null,
		inverse: false,
		invisible: false,
		italic: false,
		overline: false,
		strikethrough: false,
		underline: { kind: TerminalColorKind.none },
		underlineKind: UnderlineKind.none,
	};
	const rowText = [
		'DPR surface geometry',
		'Canvas raster state',
		'input anchor',
		'listener disposal',
	];
	const frame: TerminalFrame = {
		activeScreen: ActiveScreen.primary,
		attachmentId,
		baseRevision: null,
		cellPixelHeight: 17,
		cellPixelWidth: 8,
		columns,
		cursor: {
			blinking: false,
			column: cursor.column,
			row: cursor.row,
			shape: CursorShape.bar,
			visible: true,
			wideTail: false,
		},
		currentDirectory: '/fixture',
		defaultColors: { backgroundRgb: 0x10_20_30, cursorRgb: null, foregroundRgb: 0xf4_f4_f4 },
		extensions: [],
		generation: 1n,
		hostEpoch,
		interaction: {
			applicationCursorKeys: false,
			applicationKeypad: false,
			bracketedPaste: false,
			focusReporting: false,
			kittyDisambiguateEscapeCodes: false,
			kittyReportAllKeysAsEscapeCodes: false,
			kittyReportAlternateKeys: false,
			kittyReportAssociatedText: false,
			kittyReportEventTypes: false,
			modifyOtherKeys2: false,
			mouseAny: false,
			mouseButton: false,
			mouseNormal: false,
			mouseSgr: false,
			mouseSgrPixels: false,
			mouseUtf8: false,
			mouseUrxvt: false,
			mouseX10: false,
		},
		kind: TerminalFrameKind.snapshot,
		revision: 1n,
		rowData: rowText.map((text, row) => ({
			cells: Array.from({ length: columns }, (_, column) => ({
				grapheme: text[column] ?? '',
				protected: false,
				semanticContent: SemanticContent.output,
				style: 0,
				width: CellWidth.narrow,
			})),
			index: row,
			promptClass: RowPromptClass.none,
			wrap: false,
			wrapContinuation: false,
		})),
		rows,
		scrollbackRows: 0n,
		sequence: 1n,
		styles: [style],
		title: 'SU-08 DPR witness',
	};
	const encoded = encodeTerminalFrame(frame);
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}
