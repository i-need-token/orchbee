import {
	ProjectedTerminal,
	type ProjectedTerminalCommand,
	type ProjectedTerminalOptions,
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

class WitnessProjectedTerminal extends ProjectedTerminal {
	override async open(root: HTMLElement): Promise<void> {
		incrementDataset(root, 'witnessOpenCount');
		await super.open(root);
	}
}

const root = document.createElement('main');
root.id = 'terminal-input-repair-witness';
root.dataset['witnessCreateCount'] = '0';
root.dataset['witnessOpenCount'] = '0';
root.dataset['witnessSyntheticEventScope'] = 'application-layer-only';
root.style.boxSizing = 'border-box';
root.style.height = '240px';
root.style.overflow = 'hidden';
root.style.position = 'relative';
root.style.width = '720px';
document.body.replaceChildren(root);
document.body.style.margin = '0';

forceCanvasRendererForWitness();
incrementDataset(root, 'witnessCreateCount');
const commands: ProjectedTerminalCommand[] = [];
const options: ProjectedTerminalOptions = {
	attachmentId,
	creationId,
	hostEpoch,
	labels: { input: 'Synthetic IME witness input', screen: 'Synthetic IME witness screen' },
	port: {
		send: (command) => {
			commands.push(command);
		},
	},
	terminalId,
};
const terminal = new WitnessProjectedTerminal(options);
let renderedFrames = 0;
terminal.onFrameTelemetry((event) => {
	renderedFrames += 1;
	root.dataset['witnessRenderer'] = event.backend;
});
void runWitness();

async function runWitness(): Promise<void> {
	try {
		await terminal.open(root);
		terminal.applyFrame(terminalSnapshot());
		const input = terminal.textarea;
		const canvas = terminal.canvas;
		if (input === undefined || canvas === undefined) {
			throw new Error('Input-repair witness surface was not created.');
		}
		const initialAttachment = attachmentId;

		input.focus();
		await nextPaint();
		const baselineHash = canvasHash(canvas);

		input.value = '候选';
		input.setSelectionRange(2, 2);
		dispatchComposition(input, 'compositionstart', '候');
		dispatchComposition(input, 'compositionupdate', '候选');
		await nextPaint();
		const activeHash = canvasHash(canvas);
		if (activeHash === baselineHash) {
			throw new Error('Synthetic composition did not produce an observable preedit frame.');
		}
		const activeDrawCount = renderedFrameCount();

		terminal.reset();
		dispatchComposition(input, 'compositionupdate', '过期候选');
		await nextPaint();
		const repairedHash = canvasHash(canvas);
		const repairedDrawCount = renderedFrameCount();
		if (repairedHash !== baselineHash) {
			throw new Error('Input repair left preedit pixels on the Canvas surface.');
		}
		if (repairedDrawCount !== activeDrawCount + 1) {
			throw new Error('Input repair did not complete exactly one current Canvas draw.');
		}

		await nextPaint();
		const lateUpdateHash = canvasHash(canvas);
		const lateUpdateDrawCount = renderedFrameCount();
		if (lateUpdateHash !== repairedHash || lateUpdateDrawCount !== repairedDrawCount) {
			throw new Error('A late composition update republished stale preedit state.');
		}

		dispatchComposition(input, 'compositionend', '过期候选');
		terminal.reset();
		dispatchBeforeInput(input, '过期候选');
		const textCountAfterLateEnd = textInputCount();
		if (textCountAfterLateEnd !== 0) {
			throw new Error('Repeated repair erased cancelled composition settlement.');
		}

		dispatchComposition(input, 'compositionstart', '未结束旧输入');
		dispatchComposition(input, 'compositionupdate', '未结束旧输入');
		terminal.reset();
		dispatchComposition(input, 'compositionstart', '新');
		dispatchComposition(input, 'compositionupdate', '新输入');
		dispatchComposition(input, 'compositionend', '新输入');
		const textCountAfterNewEnd = textInputCount();
		dispatchComposition(input, 'compositionend', '新输入');
		const textCountAfterDuplicateEnd = textInputCount();
		if (textCountAfterNewEnd !== 1 || textCountAfterDuplicateEnd !== 1) {
			throw new Error('A new composition did not commit exactly once.');
		}
		if (input.value !== '') throw new Error('Composition completion did not clear the textarea.');

		const shiftedNativeStart = commands.length;
		dispatchKey(input, 'keydown', 'ShiftLeft', 'Shift', true);
		dispatchKey(input, 'keydown', 'Semicolon', ':', true);
		dispatchTextBeforeInput(input, ':');
		input.value = '：';
		dispatchTextInput(input, '：');
		dispatchKey(input, 'keyup', 'Semicolon', ':', true);
		dispatchKey(input, 'keyup', 'ShiftLeft', 'Shift', false);
		await nextTask();
		const shiftedNativeCommands = commands.slice(shiftedNativeStart);
		const shiftedNativeText = shiftedNativeCommands.flatMap((command) =>
			command.kind === 'text-input' ? [command.payload.text] : [],
		);
		const shiftedNativeKeyCount = shiftedNativeCommands.filter(
			(command) => command.kind === 'key-input' && command.payload.key === 'Semicolon',
		).length;
		if (shiftedNativeText.length !== 1 || shiftedNativeText[0] !== '：') {
			throw new Error('Shift punctuation did not preserve the native IME Unicode text.');
		}
		if (shiftedNativeKeyCount !== 0) {
			throw new Error('Shift punctuation also published a physical punctuation key.');
		}

		const shiftedFallbackStart = commands.length;
		dispatchKey(input, 'keydown', 'ShiftLeft', 'Shift', true);
		dispatchKey(input, 'keydown', 'Semicolon', ':', true);
		dispatchKey(input, 'keyup', 'Semicolon', ':', true);
		dispatchKey(input, 'keyup', 'ShiftLeft', 'Shift', false);
		await nextTask();
		const shiftedFallbackCommands = commands.slice(shiftedFallbackStart);
		const shiftedFallbackText = shiftedFallbackCommands.flatMap((command) =>
			command.kind === 'text-input' ? [command.payload.text] : [],
		);
		const shiftedFallbackKeyCount = shiftedFallbackCommands.filter(
			(command) => command.kind === 'key-input' && command.payload.key === 'Semicolon',
		).length;
		if (shiftedFallbackText.length !== 1 || shiftedFallbackText[0] !== ':') {
			throw new Error('Shift punctuation did not publish its deterministic text fallback.');
		}
		if (shiftedFallbackKeyCount !== 0) {
			throw new Error('Shift fallback published a physical punctuation key.');
		}

		if (commands.some((command) => command.payload.attachmentId !== initialAttachment)) {
			throw new Error('Input repair changed the projected attachment.');
		}

		root.dataset['witnessActivePreeditObserved'] = 'true';
		root.dataset['witnessAttachmentId'] = initialAttachment;
		root.dataset['witnessLateEndTextCount'] = String(textCountAfterLateEnd);
		root.dataset['witnessRepeatedRepairGuardTextCount'] = String(textCountAfterLateEnd);
		root.dataset['witnessLateUpdateDrawDelta'] = String(lateUpdateDrawCount - repairedDrawCount);
		root.dataset['witnessNewCompositionTextCount'] = String(textCountAfterNewEnd);
		root.dataset['witnessRestartBeforeOldEndTextCount'] = String(textCountAfterNewEnd);
		root.dataset['witnessDuplicateEndTextCount'] = String(textCountAfterDuplicateEnd);
		root.dataset['witnessRepairDrawDelta'] = String(repairedDrawCount - activeDrawCount);
		root.dataset['witnessShiftedFallbackKeyCount'] = String(shiftedFallbackKeyCount);
		root.dataset['witnessShiftedFallbackText'] = shiftedFallbackText[0];
		root.dataset['witnessShiftedNativeKeyCount'] = String(shiftedNativeKeyCount);
		root.dataset['witnessShiftedNativeText'] = shiftedNativeText[0];
		root.dataset['witnessTextareaCleared'] = String(input.value === '');
		root.dataset['witnessSameAttachment'] = 'true';
		terminal.dispose();
		root.dataset['witnessDomChildCountAfterDispose'] = String(root.childElementCount);
		root.dataset['witnessReady'] = 'true';
	} catch (error) {
		root.dataset['witnessFailure'] = error instanceof Error ? error.message : String(error);
	}
}

function forceCanvasRendererForWitness(): void {
	Object.defineProperty(navigator, 'gpu', {
		configurable: true,
		value: undefined,
	});
	root.dataset['witnessForcedCanvas'] = 'true';
}

function dispatchComposition(
	input: HTMLTextAreaElement,
	type: 'compositionend' | 'compositionstart' | 'compositionupdate',
	data: string,
): void {
	input.dispatchEvent(new CompositionEvent(type, { bubbles: true, data }));
}

function dispatchBeforeInput(input: HTMLTextAreaElement, data: string): void {
	input.dispatchEvent(
		new InputEvent('beforeinput', {
			bubbles: true,
			data,
			inputType: 'insertFromComposition',
			isComposing: false,
		}),
	);
}

function dispatchKey(
	input: HTMLTextAreaElement,
	type: 'keydown' | 'keyup',
	code: string,
	key: string,
	shiftKey: boolean,
): void {
	input.dispatchEvent(
		new KeyboardEvent(type, { bubbles: true, cancelable: true, code, key, shiftKey }),
	);
}

function dispatchTextBeforeInput(input: HTMLTextAreaElement, data: string): void {
	input.dispatchEvent(
		new InputEvent('beforeinput', {
			bubbles: true,
			cancelable: true,
			data,
			inputType: 'insertText',
			isComposing: false,
		}),
	);
}

function dispatchTextInput(input: HTMLTextAreaElement, data: string): void {
	input.dispatchEvent(
		new InputEvent('input', {
			bubbles: true,
			data,
			inputType: 'insertText',
			isComposing: false,
		}),
	);
}

function renderedFrameCount(): number {
	return renderedFrames;
}

function textInputCount(): number {
	return commands.filter((command) => command.kind === 'text-input').length;
}

function canvasHash(canvas: HTMLCanvasElement): string {
	const context = canvas.getContext('2d');
	if (context === null) throw new Error('Input-repair witness requires Canvas 2D.');
	const pixels = context.getImageData(0, 0, canvas.width, canvas.height).data;
	let hash = 2_166_136_261;
	for (const byte of pixels) {
		hash ^= byte;
		hash = Math.imul(hash, 16_777_619);
	}
	return (hash >>> 0).toString(16).padStart(8, '0');
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
	const rows = ['input repair', 'synthetic preedit', 'same attachment', 'dispose cleanup'];
	const frame: TerminalFrame = {
		activeScreen: ActiveScreen.primary,
		attachmentId,
		baseRevision: null,
		cellPixelHeight: 18,
		cellPixelWidth: 9,
		columns: 24,
		cursor: {
			blinking: false,
			column: 3,
			row: 2,
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
			alternateScroll: false,
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
		rowData: rows.map((text, row) => ({
			cells: Array.from({ length: 24 }, (_, column) => ({
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
		rows: rows.length,
		scrollbackRows: 0n,
		sequence: 1n,
		styles: [style],
		themeRevision: 0n,
		title: 'IN-02 synthetic composition witness',
	};
	const encoded = encodeTerminalFrame(frame);
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function incrementDataset(element: HTMLElement, key: string): void {
	element.dataset[key] = String(Number(element.dataset[key] ?? '0') + 1);
}

async function nextPaint(): Promise<void> {
	await new Promise<void>((resolve) => {
		requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
	});
}

async function nextTask(): Promise<void> {
	await new Promise<void>((resolve) => {
		setTimeout(resolve, 0);
	});
}
