import { ProjectedTerminal, type ProjectedTerminalOptions } from '@orchbee/terminal';
import type { ProjectedTerminalCommand } from '@orchbee/workbench-client-protocol';
import {
	ActiveScreen,
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
	CellWidth,
	CreationIdSchema,
	CursorShape,
	encodeTerminalFrame,
	HostEpochSchema,
	KeyAction,
	RowPromptClass,
	SemanticContent,
	TerminalColorKind,
	TerminalFrameKind,
	TerminalIdSchema,
	TerminalThemeStateSchema,
	UnderlineKind,
	type TerminalFrame,
} from '@orchbee/terminal-protocol';

import {
	mountTerminalFindSurface,
	type MountTerminalFindSurfaceOptions,
} from '../../terminal-find-surface';
import {
	mountTerminalTextViewer,
	type MountTerminalTextViewerOptions,
} from '../../terminal-text-viewer';
import { installProjectedTerminalShortcuts } from '../../projected-terminal-actions';
import type { ProjectedTerminalTransport } from '../../terminal-projection';
import {
	startProjectedTerminalRenderer,
	type ProjectedTerminalRendererDependencies,
} from '../startProjectedTerminalRenderer';

const attachmentId = AttachmentIdSchema.parse('22'.repeat(16));
const creationId = CreationIdSchema.parse('33'.repeat(16));
const hostEpoch = HostEpochSchema.parse('11'.repeat(16));
const terminalId = TerminalIdSchema.parse('44'.repeat(16));
const capabilityRevision = CanonicalUint64TextSchema.parse('1');
const selectionRow = CanonicalUint64TextSchema.parse('10');
const theme = TerminalThemeStateSchema.parse({
	operationId: null,
	revision: '0',
	terminal: {
		colorScheme: 'dark',
		defaultBackgroundRgb: 0x10_20_30,
		defaultCursorRgb: null,
		defaultForegroundRgb: 0xf4_f4_f4,
		palette: Array.from({ length: 256 }, (_, index) => index),
	},
});

const root = document.createElement('main');
root.id = 'terminal-input-capability-witness';
root.style.boxSizing = 'border-box';
root.style.height = '240px';
root.style.overflow = 'hidden';
root.style.position = 'relative';
root.style.width = '720px';
document.body.replaceChildren(root);
document.body.style.margin = '0';

Object.defineProperty(navigator, 'gpu', { configurable: true, value: undefined });
root.dataset['witnessForcedCanvas'] = 'true';

const commands: ProjectedTerminalCommand[] = [];
let terminal: ProjectedTerminal | undefined;
let transportListener: (value: unknown) => void = () => undefined;
let reviewSignal: AbortSignal | undefined;
const dependencies: ProjectedTerminalRendererDependencies = {
	createTerminal: (options: ProjectedTerminalOptions) => {
		terminal = new ProjectedTerminal(options);
		return terminal;
	},
	installShortcuts: installProjectedTerminalShortcuts,
	mountFindSurface: (options: MountTerminalFindSurfaceOptions) => mountTerminalFindSurface(options),
	mountTextViewer: (options: MountTerminalTextViewerOptions) => mountTerminalTextViewer(options),
};
const transport: ProjectedTerminalTransport = {
	close: () => {
		root.dataset['witnessTransportClosed'] = 'true';
	},
	projectionBufferTrust: 'local-host',
	send: (command) => {
		commands.push(command);
	},
	subscribe: (listener) => {
		transportListener = listener;
		return () => undefined;
	},
};

void runWitness();

async function runWitness(): Promise<void> {
	try {
		const session = await startProjectedTerminalRenderer(
			root,
			document.documentElement,
			['en-US'],
			'standard',
			{
				attachmentId,
				canInput: true,
				canResize: false,
				capabilityRevision,
				creationId,
				hostEpoch,
				terminalId,
				terminalOutcome: null,
				theme,
			},
			transport,
			dependencies,
			{},
			undefined,
			(_review, signal) => {
				reviewSignal = signal;
				return new Promise<boolean>(() => undefined);
			},
		);
		const activeTerminal = terminal;
		if (activeTerminal === undefined) throw new Error('Projected terminal was not created.');
		transportListener({ frame: terminalSnapshot(), kind: 'frame' });
		await nextPaint();
		const input = activeTerminal.textarea;
		const canvas = activeTerminal.canvas;
		if (input === undefined || canvas === undefined) {
			throw new Error('Projected input-capability surface was not created.');
		}

		input.focus();
		input.dispatchEvent(keyboardEvent('keydown', 'KeyA', 'a'));
		canvas.dispatchEvent(
			new PointerEvent('pointerdown', {
				bubbles: true,
				button: 0,
				buttons: 1,
				clientX: 4,
				clientY: 4,
				pointerId: 7,
				pointerType: 'mouse',
			}),
		);
		input.value = 'candidate';
		input.dispatchEvent(new CompositionEvent('compositionstart', { bubbles: true, data: 'can' }));
		input.dispatchEvent(
			new CompositionEvent('compositionupdate', { bubbles: true, data: 'candidate' }),
		);
		const pendingPaste = activeTerminal.paste('review\nrequired');
		await until(() => reviewSignal !== undefined);
		if (!hasInputCommand('key-input', KeyAction.press) || !hasInputCommand('mouse-input')) {
			throw new Error('Witness did not establish held key and mouse input before downgrade.');
		}

		transportListener({
			capabilities: {
				attachmentId,
				capabilities: { input: false, read: true, resize: true },
				capabilityRevision: '2',
				resizeLease: false,
				terminalId,
			},
			kind: 'attachment-input-capability-changed',
		});
		const inputCountAfterDowngrade = inputCommandCount();
		const readOnlyObserved = input.readOnly && !activeTerminal.canControlTerminal;
		const textareaCleared = input.value === '';
		input.dispatchEvent(keyboardEvent('keyup', 'KeyA', 'a'));
		canvas.dispatchEvent(
			new PointerEvent('pointerup', {
				bubbles: true,
				button: 0,
				buttons: 0,
				clientX: 4,
				clientY: 4,
				pointerId: 7,
				pointerType: 'mouse',
			}),
		);
		input.dispatchEvent(
			new CompositionEvent('compositionend', { bubbles: true, data: 'candidate' }),
		);
		input.dispatchEvent(
			new InputEvent('beforeinput', {
				bubbles: true,
				data: 'candidate',
				inputType: 'insertFromComposition',
			}),
		);
		await pendingPaste;

		activeTerminal.notifySelectionChanged({
			attachmentId,
			end: { column: 4, row: selectionRow },
			preview: 'read',
			rectangle: false,
			start: { column: 0, row: selectionRow },
			state: 'active',
		});
		activeTerminal.selectAll();
		activeTerminal.scrollLines(-1);
		activeTerminal.updateSearch('read-only');
		activeTerminal.copySelection();
		if (
			!commands.some((command) => command.kind === 'selection-command') ||
			!commands.some((command) => command.kind === 'read-rows') ||
			!commands.some((command) => command.kind === 'selection-copy')
		) {
			throw new Error('Read-only local output operations were not retained.');
		}
		const downgradeInputCommandDelta = inputCommandCount() - inputCountAfterDowngrade;
		root.dataset['witnessStep'] = 'local-operations-observed';

		transportListener({
			capabilities: {
				attachmentId,
				capabilities: { input: true, read: true, resize: true },
				capabilityRevision: '3',
				resizeLease: false,
				terminalId,
			},
			kind: 'attachment-input-capability-changed',
		});
		root.dataset['witnessStep'] = 'upgrade-observed';
		input.dispatchEvent(keyboardEvent('keydown', 'KeyB', 'b'));
		input.dispatchEvent(keyboardEvent('keyup', 'KeyB', 'b'));
		root.dataset['witnessStep'] = 'fresh-input-observed';
		const upgradeObserved = !input.readOnly && activeTerminal.canControlTerminal;
		const freshRevisions = commands
			.filter(
				(command): command is Extract<ProjectedTerminalCommand, { kind: 'key-input' }> =>
					command.kind === 'key-input' && command.payload.key === 'KeyB',
			)
			.map((command) => command.payload.capabilityRevision);
		if (freshRevisions.join(',') !== '3,3') {
			throw new Error('Recovered input did not use only the fresh capability revision.');
		}

		root.dataset['witnessDowngradeInputCommandDelta'] = String(downgradeInputCommandDelta);
		root.dataset['witnessFreshInputRevisions'] = freshRevisions.join(',');
		root.dataset['witnessLocalOperations'] = 'copy,history,search,selection';
		root.dataset['witnessPasteReviewAborted'] = String(reviewSignal?.aborted === true);
		root.dataset['witnessReadOnlyObserved'] = String(readOnlyObserved);
		root.dataset['witnessSearchAvailable'] = 'true';
		root.dataset['witnessTextareaCleared'] = String(textareaCleared);
		root.dataset['witnessUpgradeObserved'] = String(upgradeObserved);
		root.dataset['witnessStalePasteCount'] = String(
			commands.filter((command) => command.kind === 'paste-input').length,
		);
		root.dataset['witnessStaleTextCount'] = String(
			commands.filter((command) => command.kind === 'text-input').length,
		);
		root.dataset['witnessReady'] = 'true';
		session.dispose();
	} catch (error) {
		root.dataset['witnessFailure'] = error instanceof Error ? error.message : String(error);
	}
}

function keyboardEvent(type: 'keydown' | 'keyup', code: string, key: string): KeyboardEvent {
	return new KeyboardEvent(type, { bubbles: true, cancelable: true, code, key });
}

function inputCommandCount(): number {
	return commands.filter((command) =>
		['focus-input', 'key-input', 'mouse-input', 'paste-input', 'text-input'].includes(command.kind),
	).length;
}

function hasInputCommand(
	kind: ProjectedTerminalCommand['kind'],
	action?: (typeof KeyAction)[keyof typeof KeyAction],
): boolean {
	return commands.some(
		(command) =>
			command.kind === kind &&
			(action === undefined || (command.kind === 'key-input' && command.payload.action === action)),
	);
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
	const rows = ['live capability', 'read only repair', 'local output', 'fresh recovery'];
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
			mouseAny: true,
			mouseButton: false,
			mouseNormal: false,
			mouseSgr: true,
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
		scrollbackRows: 10n,
		sequence: 1n,
		styles: [style],
		themeRevision: 0n,
		title: 'IN-10 live input capability witness',
	};
	const encoded = encodeTerminalFrame(frame);
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

async function nextPaint(): Promise<void> {
	await new Promise<void>((resolve) => {
		requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
	});
}

async function until(predicate: () => boolean): Promise<void> {
	for (let attempt = 0; attempt < 20; attempt += 1) {
		if (predicate()) return;
		await Promise.resolve();
	}
	throw new Error('Witness condition did not become observable.');
}
