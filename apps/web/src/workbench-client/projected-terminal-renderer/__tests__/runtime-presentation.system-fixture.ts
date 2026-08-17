import {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
	ProjectedTerminal,
	type ProjectedTerminalCommand,
	type ProjectedTerminalOptions,
	type TerminalPresentationConfiguration,
} from '@orchbee/terminal';
import {
	ActiveScreen,
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
	CellWidth,
	CreationIdSchema,
	CursorShape,
	encodeTerminalFrame,
	encodeTerminalRowWindow,
	HostEpochSchema,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalFrameKind,
	TerminalIdSchema,
	TerminalRowWindowDetail,
	UnderlineKind,
	type TerminalFrame,
} from '@orchbee/terminal-protocol';

import {
	mountTerminalFindSurface,
	type MountTerminalFindSurfaceOptions,
} from '../../terminal-find-surface';
import { installProjectedTerminalShortcuts } from '../../projected-terminal-actions';
import type { ProjectedTerminalTransport } from '../../terminal-projection';
import {
	startProjectedTerminalRenderer,
	type ProjectedTerminalRendererDependencies,
} from '../startProjectedTerminalRenderer';
import type { TerminalPresentationConfigurationSource } from '../../../terminal-presentation';

const attachmentId = AttachmentIdSchema.parse('22'.repeat(16));
const creationId = CreationIdSchema.parse('33'.repeat(16));
const hostEpoch = HostEpochSchema.parse('11'.repeat(16));
const terminalId = TerminalIdSchema.parse('44'.repeat(16));
const columns = 32;
const rowText = [
	'runtime presentation',
	'search needle active',
	'bold configuration',
	'preedit anchor',
] as const;

const runtimeConfiguration: TerminalPresentationConfiguration = {
	allowTransparency: true,
	backgroundOpacity: 0.75,
	defaultColorOverride: null,
	fontFamily: 'serif',
	fontLigatures: false,
	fontMetricModifiers: { cellHeight: 5, cellWidth: 3 },
	fontSize: 22,
	fontWeight: 500,
	fontWeightBold: 800,
	paddingBalance: 'equal',
};

class MutablePresentationConfigurationSource implements TerminalPresentationConfigurationSource {
	readonly #listeners = new Set<(configuration: TerminalPresentationConfiguration) => void>();
	#value = DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION;

	current(): TerminalPresentationConfiguration {
		return this.#value;
	}

	subscribe(listener: (configuration: TerminalPresentationConfiguration) => void): () => void {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}

	emit(configuration: TerminalPresentationConfiguration): void {
		this.#value = configuration;
		for (const listener of this.#listeners) listener(configuration);
	}
}

class WitnessProjectedTerminal extends ProjectedTerminal {
	override async open(root: HTMLElement): Promise<void> {
		incrementDataset(root, 'witnessOpenCount');
		await super.open(root);
	}
}

const root = document.createElement('main');
root.id = 'terminal-presentation-witness';
root.dataset['witnessCreateCount'] = '0';
root.dataset['witnessFontPreparationCount'] = '0';
root.dataset['witnessOpenCount'] = '0';
root.dataset['witnessResizeCount'] = '0';
root.dataset['witnessSelectionCopyCount'] = '0';
root.style.boxSizing = 'border-box';
root.style.height = '320px';
root.style.overflow = 'hidden';
root.style.padding = '7px 29px 13px 17px';
root.style.position = 'relative';
root.style.width = '760px';

const prepareButton = document.createElement('button');
prepareButton.id = 'prepare-runtime-presentation';
prepareButton.textContent = 'Prepare runtime presentation';
const releaseButton = document.createElement('button');
releaseButton.id = 'release-runtime-font';
releaseButton.textContent = 'Release runtime font';
releaseButton.disabled = true;
document.body.replaceChildren(root, prepareButton, releaseButton);
document.body.style.background = '#d04161';
document.body.style.margin = '0';

const source = new MutablePresentationConfigurationSource();
let terminal: WitnessProjectedTerminal | undefined;
let transportListener: (value: unknown) => void = () => undefined;
let fontGate: Promise<void> = Promise.resolve();
let releaseFontGate: (() => void) | undefined;
let holdRuntimeFonts = false;

const nativeFontLoad = document.fonts.load.bind(document.fonts);
Object.defineProperty(document.fonts, 'load', {
	configurable: true,
	value: async (font: string, text?: string): Promise<FontFace[]> => {
		if (holdRuntimeFonts) {
			incrementDataset(root, 'witnessFontPreparationCount');
			await fontGate;
		}
		return nativeFontLoad(font, text);
	},
});

function forceCanvasRendererForWitness(): void {
	Object.defineProperty(navigator, 'gpu', {
		configurable: true,
		value: undefined,
	});
	root.dataset['witnessForcedCanvas'] = 'true';
}

const dependencies: ProjectedTerminalRendererDependencies = {
	createTerminal: (options: ProjectedTerminalOptions) => {
		forceCanvasRendererForWitness();
		incrementDataset(root, 'witnessCreateCount');
		terminal = new WitnessProjectedTerminal(options);
		terminal.onSearchResults((event) => {
			root.dataset['witnessSearchActiveIndex'] = String(event.activeIndex);
			root.dataset['witnessSearchMatchCount'] = String(event.matches.length);
			root.dataset['witnessSearchQuery'] = event.query;
			root.dataset['witnessSearchState'] = event.state;
		});
		return terminal;
	},
	installShortcuts: installProjectedTerminalShortcuts,
	mountFindSurface: (options: MountTerminalFindSurfaceOptions) => mountTerminalFindSurface(options),
};

const transport: ProjectedTerminalTransport = {
	close: () => {
		root.dataset['witnessTransportClosed'] = 'true';
	},
	projectionBufferTrust: 'local-host',
	send: (command) => {
		if (command.kind === 'clipboard-write-decision') return;
		observeCommand(command);
	},
	subscribe: (listener) => {
		transportListener = listener;
		return () => {
			transportListener = () => undefined;
		};
	},
};

prepareButton.addEventListener('click', () => {
	if (holdRuntimeFonts) return;
	holdRuntimeFonts = true;
	fontGate = new Promise<void>((resolve) => {
		releaseFontGate = resolve;
	});
	prepareButton.disabled = true;
	releaseButton.disabled = false;
	source.emit(runtimeConfiguration);
});

releaseButton.addEventListener('click', () => {
	holdRuntimeFonts = false;
	releaseFontGate?.();
	releaseFontGate = undefined;
	releaseButton.disabled = true;
});

void startWitness();

async function startWitness(): Promise<void> {
	try {
		const session = await startProjectedTerminalRenderer(
			root,
			document.documentElement,
			['en-US'],
			'apple',
			{
				attachmentId,
				canInput: true,
				canResize: true,
				creationId,
				hostEpoch,
				terminalId,
				terminalOutcome: null,
			},
			transport,
			dependencies,
			{},
			source,
		);
		transportListener({ frame: terminalSnapshot(), kind: 'frame' });
		await nextPaint();
		const currentTerminal = terminal;
		if (currentTerminal === undefined) throw new Error('Witness terminal was not created.');
		currentTerminal.notifySelectionChanged({
			attachmentId,
			end: { column: 9, row: CanonicalUint64TextSchema.parse('0') },
			preview: 'runtime',
			rectangle: false,
			start: { column: 1, row: CanonicalUint64TextSchema.parse('0') },
			state: 'active',
		});
		await prepareSearch(currentTerminal);
		const input = currentTerminal.textarea;
		if (input === undefined) throw new Error('Witness input was not created.');
		input.focus();
		input.value = '候';
		input.setSelectionRange(1, 1);
		input.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true, data: '候' }));
		input.value = '候选';
		input.setSelectionRange(2, 2);
		input.dispatchEvent(new CompositionEvent('compositionupdate', { bubbles: true, data: '候选' }));
		await nextPaint();
		root.dataset['witnessBaselineResizeCount'] = root.dataset['witnessResizeCount'] ?? '0';
		root.dataset['witnessReady'] = 'true';
		window.addEventListener(
			'pagehide',
			() => {
				session.dispose();
			},
			{ once: true },
		);
	} catch (error) {
		root.dataset['witnessFailure'] = error instanceof Error ? error.message : String(error);
	}
}

async function prepareSearch(currentTerminal: ProjectedTerminal): Promise<void> {
	await new Promise<void>((resolve, reject) => {
		const timeout = window.setTimeout(() => {
			subscription.dispose();
			reject(new Error('Witness search did not complete.'));
		}, 5_000);
		const subscription = currentTerminal.onSearchResults((event) => {
			if (event.state !== 'complete') return;
			window.clearTimeout(timeout);
			subscription.dispose();
			resolve();
		});
		currentTerminal.updateSearch('needle');
	});
}

function observeCommand(command: ProjectedTerminalCommand): void {
	if (command.kind === 'resize-terminal') {
		incrementDataset(root, 'witnessResizeCount');
		root.dataset['witnessResizeCellHeight'] = String(command.payload.cellPixelHeight);
		root.dataset['witnessResizeCellWidth'] = String(command.payload.cellPixelWidth);
		return;
	}
	if (command.kind === 'selection-copy') {
		incrementDataset(root, 'witnessSelectionCopyCount');
		return;
	}
	if (command.kind !== 'read-rows') return;
	queueMicrotask(() => {
		const rows = textRows(command);
		transportListener({ kind: 'rows', rows });
	});
}

function textRows(
	command: Extract<ProjectedTerminalCommand, { readonly kind: 'read-rows' }>,
): Uint8Array {
	if (command.payload.origin.kind !== 'absolute') {
		throw new Error('Witness search requested a non-absolute row window.');
	}
	const startRow = BigInt(command.payload.origin.row);
	const selected = rowText.slice(Number(startRow), Number(startRow) + command.payload.rowCount);
	const encoded = encodeTerminalRowWindow({
		attachmentId,
		columns,
		detail: TerminalRowWindowDetail.text,
		generation: 1n,
		hostEpoch,
		revision: 1n,
		rows: selected.map((text, offset) => ({
			index: startRow + BigInt(offset),
			mappings: [...text].map((_, index) => ({
				byteOffset: index,
				column: index,
				utf16Offset: index,
				width: 1,
			})),
			promptClass: RowPromptClass.none,
			text,
			wrap: false,
			wrapContinuation: false,
		})),
		sequence: BigInt(command.payload.requestSequence),
		startRow,
		totalRows: BigInt(rowText.length),
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function terminalSnapshot(): Uint8Array {
	const regularStyle: TerminalFrame['styles'][number] = {
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
	const frame: TerminalFrame = {
		activeScreen: ActiveScreen.primary,
		attachmentId,
		baseRevision: null,
		cellPixelHeight: 18,
		cellPixelWidth: 9,
		columns,
		cursor: {
			blinking: false,
			column: 14,
			row: 3,
			shape: CursorShape.bar,
			visible: true,
			wideTail: false,
		},
		currentDirectory: '/fixture',
		defaultColors: {
			backgroundRgb: 0x10_20_30,
			cursorRgb: null,
			foregroundRgb: 0xf4_f4_f4,
		},
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
				style: row === 2 ? 1 : 0,
				width: CellWidth.narrow,
			})),
			index: row,
			promptClass: RowPromptClass.none,
			wrap: false,
			wrapContinuation: false,
		})),
		rows: rowText.length,
		scrollbackRows: 0n,
		sequence: 1n,
		styles: [regularStyle, { ...regularStyle, bold: true }],
		title: 'SU-14 witness',
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
