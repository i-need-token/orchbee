import { ActionRuntime, type ActionDiscoveryResult, type ActionScope } from '@orchbee/actions';
import '@orchbee/ui/styles.css';

import {
	ProjectedTerminal,
	type ProjectedTerminalCommand as TerminalSurfaceCommand,
	type ProjectedTerminalOptions,
} from '@orchbee/terminal';
import type { WorkbenchTerminalActionContextProvider } from '../../../workbench';
import type { ProjectedTerminalCommand } from '@orchbee/workbench-client-protocol';
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
	TerminalGraphicsUpdateKind,
	TerminalIdSchema,
	TerminalRowWindowDetail,
	TerminalThemeStateSchema,
	UnderlineKind,
	type TerminalFrame,
} from '@orchbee/terminal-protocol';

import { createBrowserTerminalPlatformEffects } from '../../projected-terminal-platform-effects';
import {
	mountTerminalFindSurface,
	type MountTerminalFindSurfaceOptions,
} from '../../terminal-find-surface';
import {
	installProjectedTerminalShortcuts,
	PROJECTED_TERMINAL_ACTION_IDS,
} from '../../projected-terminal-actions';
import type { ProjectedTerminalTransport } from '../../terminal-projection';
import {
	startProjectedTerminalRenderer,
	type ProjectedTerminalRendererDependencies,
} from '../startProjectedTerminalRenderer';

interface LinkScenario {
	readonly id: string;
	readonly permitted: boolean;
	readonly text: string;
	readonly target?: string;
}

const scenarios: readonly LinkScenario[] = [
	{ id: 'http', permitted: true, text: 'http://orchbee.dev/http' },
	{ id: 'https', permitted: true, text: 'https://orchbee.dev/secure' },
	{ id: 'mailto', permitted: true, text: 'mailto:terminal@orchbee.dev' },
	{ id: 'auto-ftp', permitted: false, text: 'ftp://orchbee.dev/automatic' },
	{
		id: 'explicit-ftp',
		permitted: false,
		target: 'ftp://orchbee.dev/explicit',
		text: 'explicit ftp metadata',
	},
	{
		id: 'explicit-file',
		permitted: false,
		target: 'file:///tmp/orchbee-private',
		text: 'explicit file metadata',
	},
	{ id: 'detected-path', permitted: false, text: './private/file.txt' },
];
const hintsScenario: LinkScenario = {
	id: 'hints',
	permitted: true,
	text: 'https://o.dev README.md src/a.ts:42:7 deadbee',
};
const attachmentId = AttachmentIdSchema.parse('91'.repeat(16));
const emptyGraphics = {
	animationTick: 0n,
	images: [],
	kind: TerminalGraphicsUpdateKind.snapshot,
	placements: [],
	resourceRevision: 0n,
} as const;
const creationId = CreationIdSchema.parse('92'.repeat(16));
const hostEpoch = HostEpochSchema.parse('93'.repeat(16));
const terminalId = TerminalIdSchema.parse('94'.repeat(16));
const localAttachmentId = AttachmentIdSchema.parse('95'.repeat(16));
const localCreationId = CreationIdSchema.parse('96'.repeat(16));
const localHostEpoch = HostEpochSchema.parse('97'.repeat(16));
const localTerminalId = TerminalIdSchema.parse('98'.repeat(16));
const capabilityRevision = CanonicalUint64TextSchema.parse('1');
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
root.id = 'terminal-url-policy-witness';
root.style.boxSizing = 'border-box';
root.style.height = '180px';
root.style.overflow = 'hidden';
root.style.position = 'relative';
root.style.width = '720px';
document.body.replaceChildren(root);
document.body.style.margin = '0';
Object.defineProperty(navigator, 'gpu', { configurable: true, value: undefined });
root.dataset['witnessForcedCanvas'] = 'true';

const actionRuntime = new ActionRuntime();
const workbenchScope = actionRuntime.createScope('workbench.fixture.url-policy');
const openedTargets: string[] = [];
const insertedHints: string[] = [];
let terminalKeyInputs = 0;
const initialScenario = scenarios[0];
if (initialScenario === undefined) throw new Error('URL policy witness has no scenarios.');
let contextProvider: WorkbenchTerminalActionContextProvider | undefined;
let currentScenario: LinkScenario = initialScenario;
let generation = 1n;
let readSettlements = 0;
let sequence = 1n;
let terminal: ProjectedTerminal | undefined;
let transportListener: (value: unknown) => void = () => undefined;

const dependencies: ProjectedTerminalRendererDependencies = {
	createTerminal: (options: ProjectedTerminalOptions) => {
		terminal = new ProjectedTerminal(options);
		return terminal;
	},
	installShortcuts: installProjectedTerminalShortcuts,
	mountFindSurface: (options: MountTerminalFindSurfaceOptions) => mountTerminalFindSurface(options),
};
const transport: ProjectedTerminalTransport = {
	close: () => undefined,
	projectionBufferTrust: 'local-host',
	send: (command) => {
		if (command.kind === 'key-input') {
			terminalKeyInputs += 1;
			root.dataset['witnessHintKeyInputs'] = String(terminalKeyInputs);
			return;
		}
		if (command.kind === 'paste-input') {
			insertedHints.push(command.payload.text);
			root.dataset['witnessHintInsertions'] = insertedHints.join('|');
			return;
		}
		if (command.kind !== 'read-rows') return;
		queueMicrotask(() => {
			transportListener({ kind: 'rows', rows: terminalRows(command) });
			readSettlements += 1;
		});
	},
	subscribe: (listener) => {
		transportListener = listener;
		return () => undefined;
	},
};

void runWitness();

async function runWitness(): Promise<void> {
	try {
		const platformEffects = createBrowserTerminalPlatformEffects({
			getClipboardTextWriter: () => undefined,
			hasTransientUserActivation: () => navigator.userActivation?.isActive === true,
			isSecureContext: () => window.isSecureContext,
			openExternal: (target) => {
				openedTargets.push(target);
				root.dataset['witnessOpenedTargets'] = openedTargets.join('|');
			},
		});
		const blockedFileEffect = await platformEffects.openExternal?.('file:///tmp/direct-effect');
		if (blockedFileEffect?.kind !== 'failed' || blockedFileEffect.code !== 'link-policy-blocked') {
			throw new Error('Browser effect did not reject direct filesystem activation.');
		}
		const session = await startProjectedTerminalRenderer(
			root,
			document.documentElement,
			['en-US'],
			'apple',
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
			platformEffects,
			undefined,
			undefined,
			undefined,
			undefined,
			undefined,
			undefined,
			{
				admitBellEffect: () => true,
				bindContextProvider: (provider) => {
					contextProvider = provider;
					return {
						dispose: () => {
							if (contextProvider === provider) contextProvider = undefined;
						},
					};
				},
				runtime: actionRuntime,
				scope: workbenchScope,
			},
		);
		const activeTerminal = terminal;
		const activeProvider = contextProvider;
		if (activeTerminal === undefined || activeProvider === undefined) {
			throw new Error('URL policy fixture did not bind its terminal Action context.');
		}
		const canvas = activeTerminal.canvas;
		if (canvas === undefined) throw new Error('URL policy fixture has no terminal canvas.');

		const permittedHover: string[] = [];
		const permittedContext: string[] = [];
		const permittedActivation: string[] = [];
		const blockedHover: string[] = [];
		const blockedContext: string[] = [];
		const blockedActivation: string[] = [];
		for (const [index, scenario] of scenarios.entries()) {
			currentScenario = scenario;
			generation = BigInt(index + 1);
			sequence += 1n;
			transportListener({ frame: terminalSnapshot(), kind: 'frame' });
			await nextPaint();
			const point = surfacePoint(canvas);
			const readsBefore = readSettlements;
			canvas.dispatchEvent(
				new PointerEvent('pointermove', {
					bubbles: true,
					buttons: 0,
					clientX: point.x,
					clientY: point.y,
					...linkModifierFacts(),
					pointerType: 'mouse',
				}),
			);
			await until(() => readSettlements > readsBefore);
			await nextPaint();
			const capture = activeProvider.capture({
				altKey: false,
				clientX: point.x,
				clientY: point.y,
				...linkModifierFacts(),
				shiftKey: false,
			});
			if (capture.kind !== 'product') {
				throw new Error(`URL policy scenario ${scenario.id} was not product-owned.`);
			}
			const hasLinkContext =
				capture.actionInputs?.[PROJECTED_TERMINAL_ACTION_IDS.openLink] !== undefined;
			const linkAction = discoverLinkAction(capture, workbenchScope);
			const opensBefore = openedTargets.length;
			if (scenario.permitted) {
				if (canvas.style.cursor !== 'pointer') {
					throw new Error(`Permitted URL ${scenario.id} did not expose link hover.`);
				}
				permittedHover.push(scenario.id);
				if (
					!hasLinkContext ||
					linkAction?.kind !== 'ready' ||
					linkAction.availability.kind !== 'enabled'
				) {
					throw new Error(`Permitted URL ${scenario.id} did not expose an enabled context Action.`);
				}
				permittedContext.push(scenario.id);
				const contextResult = await activeProvider.executePrepared(linkAction.invocation)
					.completion;
				if (contextResult.kind !== 'completed') {
					throw new Error(`Permitted URL ${scenario.id} context activation did not complete.`);
				}
				canvas.dispatchEvent(modifierClick(point));
				await until(() => openedTargets.length === opensBefore + 2);
				permittedActivation.push(scenario.id);
			} else {
				if (canvas.style.cursor === 'pointer') {
					throw new Error(`Blocked target ${scenario.id} exposed link hover.`);
				}
				blockedHover.push(scenario.id);
				if (
					hasLinkContext ||
					(linkAction?.kind === 'ready' && linkAction.availability.kind === 'enabled')
				) {
					throw new Error(`Blocked target ${scenario.id} exposed an enabled context Action.`);
				}
				blockedContext.push(scenario.id);
				canvas.dispatchEvent(modifierClick(point));
				await nextPaint();
				if (openedTargets.length !== opensBefore) {
					throw new Error(`Blocked target ${scenario.id} produced an external effect.`);
				}
				blockedActivation.push(scenario.id);
			}
		}

		root.dataset['witnessAllowedHover'] = permittedHover.join(',');
		root.dataset['witnessAllowedContext'] = permittedContext.join(',');
		root.dataset['witnessAllowedActivation'] = permittedActivation.join(',');
		root.dataset['witnessBlockedHover'] = blockedHover.join(',');
		root.dataset['witnessBlockedContext'] = blockedContext.join(',');
		root.dataset['witnessBlockedActivation'] = blockedActivation.join(',');
		root.dataset['witnessOpenedTargets'] = openedTargets.join('|');
		root.dataset['witnessDirectFileEffect'] = `${blockedFileEffect.kind}:${blockedFileEffect.code}`;
		root.dataset['witnessHintKeyInputs'] = String(terminalKeyInputs);
		currentScenario = hintsScenario;
		generation += 1n;
		sequence += 1n;
		transportListener({ frame: terminalSnapshot(), kind: 'frame' });
		await nextPaint();
		await runLocalFileEffectsWitness();
		root.dataset['witnessHintsReady'] = 'true';
		root.dataset['witnessReady'] = 'true';
		window.addEventListener(
			'pagehide',
			() => {
				session.dispose();
				actionRuntime.dispose();
			},
			{ once: true },
		);
	} catch (error) {
		root.dataset['witnessFailure'] = error instanceof Error ? error.message : String(error);
	}
}

async function runLocalFileEffectsWitness(): Promise<void> {
	const surface = document.createElement('section');
	surface.id = 'local-file-effects-witness';
	surface.style.background = '#102030';
	surface.style.boxSizing = 'border-box';
	surface.style.height = '180px';
	surface.style.position = 'relative';
	surface.style.width = '720px';
	document.body.appendChild(surface);

	const localText = './images/old.png ./images/final.png';
	const openedPaths: string[] = [];
	let localTerminal: ProjectedTerminal;
	const localPort = {
		send: (command: TerminalSurfaceCommand): void => {
			if (command.kind === 'read-rows') {
				queueMicrotask(() => localTerminal.applyRows(localTerminalRows(command, localText)));
				return;
			}
			if (command.kind === 'link-open' && command.payload.target.kind === 'file') {
				openedPaths.push(command.payload.target.path);
				root.dataset['witnessLocalOpenedPaths'] = openedPaths.join('|');
				localTerminal.notifyLinkOpened(command.payload.requestSequence);
			}
		},
	};
	localTerminal = new ProjectedTerminal({
		attachmentId: localAttachmentId,
		canResize: false,
		capabilityRevision,
		creationId: localCreationId,
		hostEpoch: localHostEpoch,
		labels: { input: 'Terminal input', screen: 'Terminal screen' },
		localFileEffects: true,
		port: localPort,
		projectionBufferTrust: 'local-host',
		terminalId: localTerminalId,
	});
	const linkSubscription = localTerminal.onLinkOpenRequest((request) => {
		localTerminal.openTarget(request.target);
	});
	await localTerminal.open(surface);
	localTerminal.applyFrame(localTerminalSnapshot(localText));
	await nextPaint();
	const canvas = localTerminal.canvas;
	if (canvas === undefined) throw new Error('Local-file witness has no terminal canvas.');
	const finalPoint = surfaceColumnPoint(canvas, localText.indexOf('./images/final.png') + 1);
	canvas.dispatchEvent(
		new PointerEvent('pointermove', {
			bubbles: true,
			buttons: 0,
			clientX: finalPoint.x,
			clientY: finalPoint.y,
			pointerType: 'mouse',
			...linkModifierFacts(),
		}),
	);
	await until(() => canvas.style.cursor === 'pointer');
	canvas.dispatchEvent(
		new MouseEvent('click', {
			bubbles: true,
			cancelable: true,
			clientX: finalPoint.x,
			clientY: finalPoint.y,
			...linkModifierFacts(),
		}),
	);
	await until(() => openedPaths.length === 1);
	root.dataset['witnessLocalReady'] = 'true';
	window.addEventListener(
		'pagehide',
		() => {
			linkSubscription.dispose();
			localTerminal.dispose();
		},
		{ once: true },
	);
}

function localTerminalRows(
	command: Extract<TerminalSurfaceCommand, { readonly kind: 'read-rows' }>,
	text: string,
): Uint8Array {
	if (command.payload.origin.kind !== 'absolute') {
		throw new Error('Local-file witness requested a non-absolute row window.');
	}
	const startRow = BigInt(command.payload.origin.row);
	const requestSequence = BigInt(command.payload.requestSequence);
	const frame = localTerminalFrame(text, requestSequence);
	const rows = startRow === 0n ? 1 : 0;
	const encoded = encodeTerminalRowWindow({
		attachmentId: localAttachmentId,
		columns: frame.columns,
		detail: TerminalRowWindowDetail.render,
		frame: { ...frame, rowData: rows === 0 ? [] : frame.rowData, rows, scrollbackRows: startRow },
		generation: 1n,
		graphics: emptyGraphics,
		hostEpoch: localHostEpoch,
		revision: 1n,
		sequence: requestSequence,
		startRow,
		totalRows: 1n,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function localTerminalSnapshot(text: string): Uint8Array {
	const encoded = encodeTerminalFrame(localTerminalFrame(text, 1n));
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function localTerminalFrame(text: string, frameSequence: bigint): TerminalFrame {
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
	return {
		activeScreen: ActiveScreen.primary,
		attachmentId: localAttachmentId,
		baseRevision: null,
		cellPixelHeight: 18,
		cellPixelWidth: 9,
		columns: 64,
		cursor: {
			blinking: false,
			column: 0,
			row: 0,
			shape: CursorShape.bar,
			visible: true,
			wideTail: false,
		},
		currentDirectory: '/fixture',
		defaultColors: { backgroundRgb: 0x10_20_30, cursorRgb: null, foregroundRgb: 0xf4_f4_f4 },
		extensions: [],
		generation: 1n,
		hostEpoch: localHostEpoch,
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
		rowData: [
			{
				cells: Array.from({ length: 64 }, (_, column) => ({
					grapheme: text[column] ?? '',
					protected: false,
					semanticContent: SemanticContent.output,
					style: 0,
					width: CellWidth.narrow,
				})),
				index: 0,
				promptClass: RowPromptClass.none,
				wrap: false,
				wrapContinuation: false,
			},
		],
		rows: 1,
		scrollbackRows: 0n,
		sequence: frameSequence,
		styles: [regularStyle],
		themeRevision: 0n,
		title: 'FS-19 local file effects witness',
	};
}

function discoverLinkAction(
	capture: Extract<
		ReturnType<WorkbenchTerminalActionContextProvider['capture']>,
		{ kind: 'product' }
	>,
	scope: ActionScope,
): ActionDiscoveryResult | undefined {
	return actionRuntime
		.discover(
			{
				...(capture.focus === undefined ? {} : { focus: capture.focus }),
				scope,
				source: 'context-menu',
				...(capture.target === undefined ? {} : { target: capture.target }),
			},
			{ inputByActionId: capture.actionInputs ?? {} },
		)
		.find((result) => result.descriptor.id === PROJECTED_TERMINAL_ACTION_IDS.openLink);
}

function modifierClick(point: { readonly x: number; readonly y: number }): MouseEvent {
	return new MouseEvent('click', {
		bubbles: true,
		cancelable: true,
		clientX: point.x,
		clientY: point.y,
		...linkModifierFacts(),
	});
}

function linkModifierFacts(): { readonly ctrlKey: boolean; readonly metaKey: boolean } {
	const apple = /Mac|iPhone|iPad/u.test(globalThis.navigator.platform);
	return { ctrlKey: !apple, metaKey: apple };
}

function surfacePoint(canvas: HTMLCanvasElement): { readonly x: number; readonly y: number } {
	const bounds = canvas.getBoundingClientRect();
	return { x: bounds.left + 4, y: bounds.top + 4 };
}

function surfaceColumnPoint(
	canvas: HTMLCanvasElement,
	column: number,
): { readonly x: number; readonly y: number } {
	const bounds = canvas.getBoundingClientRect();
	return { x: bounds.left + column * 9 + 4, y: bounds.top + 4 };
}

function terminalRows(
	command: Extract<ProjectedTerminalCommand, { readonly kind: 'read-rows' }>,
): Uint8Array {
	if (command.payload.origin.kind !== 'absolute') {
		throw new Error('URL policy witness requested a non-absolute row window.');
	}
	const startRow = BigInt(command.payload.origin.row);
	const requestSequence = BigInt(command.payload.requestSequence);
	const frame = terminalFrame(true, requestSequence);
	const rows = startRow === 0n ? 1 : 0;
	const encoded = encodeTerminalRowWindow({
		attachmentId,
		columns: frame.columns,
		detail: TerminalRowWindowDetail.render,
		frame: {
			...frame,
			rowData: rows === 0 ? [] : frame.rowData,
			rows,
			scrollbackRows: startRow,
		},
		generation,
		graphics: emptyGraphics,
		hostEpoch,
		revision: 1n,
		sequence: requestSequence,
		startRow,
		totalRows: 1n,
	});
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function terminalSnapshot(): Uint8Array {
	const encoded = encodeTerminalFrame(terminalFrame(false));
	if (!encoded.ok) throw new Error(encoded.detail);
	return encoded.value;
}

function terminalFrame(linkDetail: boolean, frameSequence = sequence): TerminalFrame {
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
	const text = currentScenario.text;
	const hyperlink = linkDetail ? (currentScenario.target ?? null) : null;
	return {
		activeScreen: ActiveScreen.primary,
		attachmentId,
		baseRevision: null,
		cellPixelHeight: 18,
		cellPixelWidth: 9,
		columns: 64,
		cursor: {
			blinking: false,
			column: 0,
			row: 0,
			shape: CursorShape.bar,
			visible: true,
			wideTail: false,
		},
		currentDirectory: '/fixture',
		defaultColors: { backgroundRgb: 0x10_20_30, cursorRgb: null, foregroundRgb: 0xf4_f4_f4 },
		extensions: [],
		generation,
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
		rowData: [
			{
				cells: Array.from({ length: 64 }, (_, column) => ({
					grapheme: text[column] ?? '',
					protected: false,
					semanticContent: SemanticContent.output,
					style: hyperlink === null || column >= text.length ? 0 : 1,
					width: CellWidth.narrow,
				})),
				index: 0,
				promptClass: RowPromptClass.none,
				wrap: false,
				wrapContinuation: false,
			},
		],
		rows: 1,
		scrollbackRows: 0n,
		sequence: frameSequence,
		styles: [regularStyle, { ...regularStyle, hyperlink }],
		themeRevision: 0n,
		title: 'FS-05 FS-06 FS-07 URL policy and hints witness',
	};
}

async function nextPaint(): Promise<void> {
	await new Promise<void>((resolve) => {
		requestAnimationFrame(() => requestAnimationFrame(() => resolve()));
	});
}

async function until(predicate: () => boolean): Promise<void> {
	for (let attempt = 0; attempt < 80; attempt += 1) {
		if (predicate()) return;
		await new Promise<void>((resolve) => setTimeout(resolve, 10));
	}
	throw new Error('URL policy witness condition did not become observable.');
}
