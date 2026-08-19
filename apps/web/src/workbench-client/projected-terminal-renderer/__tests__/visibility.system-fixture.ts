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
const animationWindowMilliseconds = 600;

const root = document.createElement('main');
root.id = 'terminal-visibility-witness';
root.dataset['witnessControl'] = 'synthetic-application-layer-document-visibility';
root.dataset['witnessLimitation'] = 'does-not-exercise-os-tab-background-throttling';
root.dataset['witnessRenderCount'] = '0';
root.style.boxSizing = 'border-box';
root.style.height = '240px';
root.style.overflow = 'hidden';
root.style.position = 'relative';
root.style.width = '720px';

const clearVisibilityButton = document.createElement('button');
clearVisibilityButton.disabled = true;
clearVisibilityButton.id = 'clear-visibility-pause-cause';
clearVisibilityButton.textContent = 'Clear synthetic visibility pause cause';

const clearDemandButton = document.createElement('button');
clearDemandButton.disabled = true;
clearDemandButton.id = 'clear-demand-pause-cause';
clearDemandButton.textContent = 'Clear explicit demand pause cause';

document.body.replaceChildren(root, clearVisibilityButton, clearDemandButton);
document.body.style.margin = '0';

let syntheticVisibilityState: DocumentVisibilityState = 'visible';
Object.defineProperty(document, 'visibilityState', {
	configurable: true,
	get: () => syntheticVisibilityState,
});
Object.defineProperty(navigator, 'gpu', { configurable: true, value: undefined });
root.dataset['witnessForcedCanvas'] = 'true';

const commands: ProjectedTerminalCommand[] = [];
const renderedFrames: ProjectedTerminalFrameTelemetry[] = [];
let awaitingInitialDraw = false;
let suspendedRenderBaseline = 0;
const terminal = new ProjectedTerminal({
	attachmentId,
	creationId,
	hostEpoch,
	labels: { input: 'Visibility witness input', screen: 'Visibility witness screen' },
	port: {
		send: (command) => {
			commands.push(command);
		},
	},
	terminalId,
});

terminal.onFrameTelemetry((event) => {
	renderedFrames.push(event);
	root.dataset['witnessRenderCount'] = String(renderedFrames.length);
	root.dataset['witnessRenderer'] = event.backend;
	root.dataset['witnessLastGeneration'] = event.generation;
	root.dataset['witnessLastRevision'] = event.revision;
	if (!awaitingInitialDraw) return;
	awaitingInitialDraw = false;
	try {
		beginJointPause(event);
		void finishJointPause().catch(reportFailure);
	} catch (error) {
		reportFailure(error);
	}
});

clearVisibilityButton.addEventListener('click', () => {
	clearVisibilityButton.disabled = true;
	void clearVisibilityCause().catch(reportFailure);
});

clearDemandButton.addEventListener('click', () => {
	clearDemandButton.disabled = true;
	void clearDemandCauseAndDispose().catch(reportFailure);
});

void startWitness();

async function startWitness(): Promise<void> {
	try {
		await terminal.open(root);
		awaitingInitialDraw = true;
		if (
			terminal.applyFrame(terminalFrame('snapshot', 1n, 1n, 1n, null, 'initial')) !== 'committed'
		) {
			throw new Error('Initial visibility witness frame did not commit.');
		}
	} catch (error) {
		reportFailure(error);
	}
}

function beginJointPause(initialFrame: ProjectedTerminalFrameTelemetry): void {
	if (
		renderedFrames.length !== 1 ||
		initialFrame.backend !== 'canvas2d' ||
		initialFrame.generation !== '1' ||
		initialFrame.revision !== '1'
	) {
		throw new Error('Initial visibility witness draw was not the current Canvas frame.');
	}
	assertAcknowledgements(['1:1:1']);
	root.dataset['witnessInitialRenderCount'] = String(renderedFrames.length);
	root.dataset['witnessInitialAcknowledgementCount'] = String(frameAcknowledgements().length);
	root.dataset['witnessInitialGeneration'] = initialFrame.generation;
	root.dataset['witnessInitialRevision'] = initialFrame.revision;

	suspendedRenderBaseline = renderedFrames.length;
	if (terminal.applyFrame(terminalFrame('delta', 2n, 2n, 1n, 1n, 'queued')) !== 'committed') {
		throw new Error('Queued visibility witness revision did not commit.');
	}
	setSyntheticVisibility('hidden');
	terminal.setProjectionDemand('paused');
	if (terminal.applyFrame(terminalFrame('delta', 3n, 3n, 1n, 2n, 'paused')) !== 'committed') {
		throw new Error('Paused visibility witness delta did not commit.');
	}
	if (terminal.applyFrame(terminalFrame('snapshot', 4n, 4n, 2n, null, 'newest')) !== 'committed') {
		throw new Error('Paused visibility witness generation did not commit.');
	}
}

async function finishJointPause(): Promise<void> {
	await delay(animationWindowMilliseconds);
	await nextPaint();
	if (renderedFrames.length !== suspendedRenderBaseline) {
		throw new Error('Joint pause allowed a queued or blinking Canvas draw.');
	}
	const metadata = terminal.metadata;
	if (metadata?.generation !== 2n || metadata.revision !== 4n) {
		throw new Error('Joint pause did not retain the newest committed packed state.');
	}
	assertAcknowledgements(['1:1:1', '1:2:2', '1:3:3', '2:4:4']);
	assertProjectionDemands(['active', 'paused']);
	root.dataset['witnessJointPauseAnimationWindowMs'] = String(animationWindowMilliseconds);
	root.dataset['witnessJointPauseCommittedCount'] = '3';
	root.dataset['witnessJointPauseAcknowledgementCount'] = String(frameAcknowledgements().length);
	root.dataset['witnessJointPauseRenderDelta'] = String(
		renderedFrames.length - suspendedRenderBaseline,
	);
	root.dataset['witnessJointPauseGeneration'] = metadata.generation.toString();
	root.dataset['witnessJointPauseRevision'] = metadata.revision.toString();
	root.dataset['witnessJointPauseComplete'] = 'true';
	root.dataset['witnessReady'] = 'true';
	clearVisibilityButton.disabled = false;
}

async function clearVisibilityCause(): Promise<void> {
	const renderBaseline = renderedFrames.length;
	setSyntheticVisibility('visible');
	await delay(animationWindowMilliseconds);
	await nextPaint();
	if (renderedFrames.length !== renderBaseline) {
		throw new Error('Clearing visibility drew while explicit demand remained paused.');
	}
	if (terminal.metadata?.generation !== 2n || terminal.metadata.revision !== 4n) {
		throw new Error('Clearing visibility changed the newest committed packed state.');
	}
	assertProjectionDemands(['active', 'paused']);
	root.dataset['witnessClearOneAnimationWindowMs'] = String(animationWindowMilliseconds);
	root.dataset['witnessClearOneRenderDelta'] = String(renderedFrames.length - renderBaseline);
	root.dataset['witnessClearOneComplete'] = 'true';
	clearDemandButton.disabled = false;
}

async function clearDemandCauseAndDispose(): Promise<void> {
	const renderBaseline = renderedFrames.length;
	terminal.setProjectionDemand('active');
	await waitForRenderCount(renderBaseline + 1);
	const resumedFrame = renderedFrames.at(-1);
	if (
		resumedFrame?.backend !== 'canvas2d' ||
		resumedFrame.generation !== '2' ||
		resumedFrame.revision !== '4'
	) {
		throw new Error('Final resume did not draw the newest Canvas generation and revision.');
	}
	await delay(100);
	await nextPaint();
	if (renderedFrames.length !== renderBaseline + 1) {
		throw new Error('Final resume scheduled more than one immediate current-state draw.');
	}
	assertProjectionDemands(['active', 'paused', 'active']);
	root.dataset['witnessResumeRenderDelta'] = String(renderedFrames.length - renderBaseline);
	root.dataset['witnessResumeGeneration'] = resumedFrame.generation;
	root.dataset['witnessResumeRevision'] = resumedFrame.revision;
	root.dataset['witnessSecondRenderDelta'] = String(renderedFrames.length - (renderBaseline + 1));
	root.dataset['witnessResumeComplete'] = 'true';

	terminal.dispose();
	const renderCountAfterDispose = renderedFrames.length;
	const commandCountAfterDispose = commands.length;
	setSyntheticVisibility('hidden');
	terminal.setProjectionDemand('active');
	const postDisposeResult = terminal.applyFrame(
		terminalFrame('snapshot', 5n, 5n, 3n, null, 'disposed'),
	);
	await delay(animationWindowMilliseconds);
	await nextPaint();
	if (
		postDisposeResult !== 'disposed' ||
		renderedFrames.length !== renderCountAfterDispose ||
		commands.length !== commandCountAfterDispose ||
		root.childElementCount !== 0
	) {
		throw new Error('Disposed visibility witness remained externally mutable.');
	}
	root.dataset['witnessDisposeAnimationWindowMs'] = String(animationWindowMilliseconds);
	root.dataset['witnessDisposeChildCount'] = String(root.childElementCount);
	root.dataset['witnessDisposeCommandDelta'] = String(commands.length - commandCountAfterDispose);
	root.dataset['witnessDisposeRenderDelta'] = String(
		renderedFrames.length - renderCountAfterDispose,
	);
	root.dataset['witnessPostDisposeApplyResult'] = postDisposeResult;
	root.dataset['witnessComplete'] = 'true';
}

function setSyntheticVisibility(state: DocumentVisibilityState): void {
	syntheticVisibilityState = state;
	document.dispatchEvent(new Event('visibilitychange'));
	root.dataset['witnessSyntheticVisibilityState'] = state;
}

function frameAcknowledgements(): Extract<
	ProjectedTerminalCommand,
	{ readonly kind: 'frame-applied' }
>[] {
	return commands.filter((command) => command.kind === 'frame-applied');
}

function assertAcknowledgements(expected: readonly string[]): void {
	const actual = frameAcknowledgements().map(
		(command) =>
			`${command.payload.generation}:${command.payload.revision}:${command.payload.sequence}`,
	);
	if (
		actual.length !== expected.length ||
		actual.some((value, index) => value !== expected[index])
	) {
		throw new Error(`Visibility witness acknowledgements diverged: ${actual.join(',')}.`);
	}
}

function assertProjectionDemands(expected: readonly string[]): void {
	const actual = commands
		.filter((command) => command.kind === 'set-projection-demand')
		.map((command) => command.payload.demand);
	if (
		actual.length !== expected.length ||
		actual.some((value, index) => value !== expected[index])
	) {
		throw new Error(`Visibility witness projection demands diverged: ${actual.join(',')}.`);
	}
}

function terminalFrame(
	kind: 'delta' | 'snapshot',
	sequence: bigint,
	revision: bigint,
	generation: bigint,
	baseRevision: bigint | null,
	marker: string,
): Uint8Array {
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
	const textRows = [marker, 'joint visibility demand', 'packed acknowledgements', 'newest only'];
	const frame: TerminalFrame = {
		activeScreen: ActiveScreen.primary,
		attachmentId,
		baseRevision,
		cellPixelHeight: 18,
		cellPixelWidth: 9,
		columns,
		cursor: {
			blinking: true,
			column: 3,
			row: 2,
			shape: CursorShape.block,
			visible: true,
			wideTail: false,
		},
		currentDirectory: '/fixture',
		defaultColors: { backgroundRgb: 0x10_20_30, cursorRgb: null, foregroundRgb: 0xf4_f4_f4 },
		extensions: [],
		generation,
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
		kind: kind === 'snapshot' ? TerminalFrameKind.snapshot : TerminalFrameKind.delta,
		revision,
		rowData: textRows.map((text, row) => ({
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
		sequence,
		styles: [style],
		themeRevision: 0n,
		title: 'SU-15 synthetic visibility witness',
	};
	const encoded = encodeTerminalFrame(frame);
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

async function waitForRenderCount(expected: number): Promise<void> {
	const deadline = performance.now() + 2_000;
	while (renderedFrames.length < expected && performance.now() < deadline) {
		await nextPaint();
	}
	if (renderedFrames.length !== expected) {
		throw new Error(
			`Visibility witness expected ${expected} renders, saw ${renderedFrames.length}.`,
		);
	}
}

async function delay(milliseconds: number): Promise<void> {
	await new Promise<void>((resolve) => {
		globalThis.setTimeout(resolve, milliseconds);
	});
}

async function nextPaint(): Promise<void> {
	await new Promise<void>((resolve) => {
		requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
	});
}

function reportFailure(error: unknown): void {
	root.dataset['witnessFailure'] = error instanceof Error ? error.message : String(error);
}
