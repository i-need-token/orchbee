import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

import {
	TerminalProcessInfoSchema,
	TerminalThemeResultSchema,
	AttachmentAcknowledgedSchema,
	AttachmentCapabilitiesSchema,
	ClaimResizeLeaseSchema,
	CURRENT_PROTOCOL_VERSION,
	decodeKeyInput,
	decodePasteInput,
	decodeTextInput,
	EmptyControlSchema,
	encodeControlJson,
	encodeControlRequest,
	encodeInputRequest,
	encodeProtocolEnvelope,
	encodeSelectionExportChunk,
	encodeTerminalProjectionBuffer,
	encodeTerminalRowWindow,
	encodeTerminalFrame,
	HelloAcceptedSchema,
	HistoryChangedSchema,
	HistoryDemandAcceptedSchema,
	InputAcceptedSchema,
	KEY_INPUT_HEADER_BYTES,
	ListTerminalsSchema,
	MessageKind,
	PROTOCOL_LIMITS,
	ProjectionBlockedSchema,
	ProjectionDemandAcceptedSchema,
	ProtocolRejectedSchema,
	ResizeAcceptedSchema,
	ResizeLeaseChangedSchema,
	SelectionChangedSchema,
	SelectionExportSchema,
	SelectionExportFinishedSchema,
	SelectionExportStartedSchema,
	SetTerminalDefaultsSchema,
	TerminalAcknowledgedSchema,
	TerminalApplicationSignalSchema,
	TerminalBellSchema,
	TerminalClipboardWriteRequestSchema,
	TerminalConfigurationAcceptedSchema,
	TerminalControlAcceptedSchema,
	TerminalDefaultsAcceptedSchema,
	TerminalExitedSchema,
	TerminalFailedSchema,
	TerminalIdentitySchema,
	TerminalListSchema,
	TerminalProjectionSection,
	TERMINAL_ROW_WINDOW_FORMAT,
	TerminalStartAcceptedSchema,
	TerminalStartedSchema,
	TerminalTargetSchema,
	TypedFailureSchema,
	UpdateAttachmentInputCapabilitySchema,
} from '@orchbee/terminal-protocol';

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const fixturesDirectory = path.resolve(packageRoot, '../../schemas/terminal/v1/fixtures');

function versionText(version) {
	return `${version.major}.${version.minor}.${version.patch}`;
}

const rowWindowVersion = versionText(TERMINAL_ROW_WINDOW_FORMAT);
const canonicalPath = path.join(fixturesDirectory, 'canonical.json');
const framePath = path.join(fixturesDirectory, `terminal-frame.bin`);
const projectionPath = path.join(fixturesDirectory, `terminal-projection-buffer.bin`);
const envelopePath = path.join(fixturesDirectory, `transport-envelope.bin`);
const inputPath = path.join(fixturesDirectory, `write-input.bin`);
const keyInputPath = path.join(fixturesDirectory, `key-input.bin`);
const textInputPath = path.join(fixturesDirectory, `text-input.bin`);
const mouseInputPath = path.join(fixturesDirectory, `mouse-input.bin`);
const focusInputPath = path.join(fixturesDirectory, `focus-input.bin`);
const pasteInputPath = path.join(fixturesDirectory, `paste-input.bin`);
const pasteAutoInputPath = path.join(fixturesDirectory, `paste-auto-input.bin`);
const promptLineInputPath = path.join(fixturesDirectory, `prompt-line-input.bin`);
const selectionExportChunkPath = path.join(fixturesDirectory, `selection-export-chunk.bin`);
const terminalFailedPath = path.join(fixturesDirectory, `terminal-failed.json`);
const controlConformancePath = path.join(fixturesDirectory, `control-conformance.json`);
const utf8BomConformancePath = path.join(fixturesDirectory, `utf8-bom-conformance.json`);
const malformedPath = path.join(fixturesDirectory, `malformed-frame.json`);
const rowWindowPath = path.join(fixturesDirectory, `terminal-row-window.bin`);
const malformedRowWindowPath = path.join(fixturesDirectory, `malformed-row-window.json`);
const packedProjectionConformancePath = path.join(
	fixturesDirectory,
	`packed-projection-conformance.json`,
);
const otherGenerationConformancePath = path.join(
	fixturesDirectory,
	`other-generation-conformance.json`,
);
const checkOnly = process.argv.includes('--check');

/**
 * PromptLineInput has no TypeScript codec: only the Worker sends it. This writes the reference
 * layout directly: attachment ID, capability revision, flags with bit 0 for Right, three
 * reserved zero bytes, the content length, then the UTF-8 content.
 */
function promptLineInputFixture(record) {
	const content = new TextEncoder().encode(record.content);
	const bytes = new Uint8Array(32 + content.byteLength);
	const view = new DataView(bytes.buffer);
	bytes.set(Buffer.from(record.attachmentId, 'hex'), 0);
	view.setBigUint64(16, BigInt(record.capabilityRevision), true);
	view.setUint8(24, record.rightArrow ? 1 : 0);
	view.setUint32(28, content.byteLength, true);
	bytes.set(content, 32);
	return bytes;
}

function bytesFromBase64(value) {
	return new Uint8Array(Buffer.from(value, 'base64'));
}

function frameFromJson(value) {
	return {
		...value,
		baseRevision: value.baseRevision === null ? null : BigInt(value.baseRevision),
		extensions: value.extensions.map((extension) => ({
			data: bytesFromBase64(extension.dataBase64),
			kind: extension.kind,
			required: extension.required,
		})),
		generation: BigInt(value.generation),
		revision: BigInt(value.revision),
		scrollbackRows: BigInt(value.scrollbackRows),
		sequence: BigInt(value.sequence),
		themeRevision: BigInt(value.themeRevision),
	};
}

function rowWindowFromJson(value) {
	return {
		...value,
		generation: BigInt(value.generation),
		revision: BigInt(value.revision),
		rows: value.rows.map((row) => ({ ...row, index: BigInt(row.index) })),
		sequence: BigInt(value.sequence),
		startRow: BigInt(value.startRow),
		totalRows: BigInt(value.totalRows),
	};
}

function encodeOrThrow(result, name) {
	if (!result.ok)
		throw new Error(`${name} fixture failed to encode: ${result.code}: ${result.detail}`);
	return result.value;
}

function controlText(schema, value, name) {
	const encoded = encodeControlJson(value, schema);
	if (!encoded.ok)
		throw new Error(`${name} control fixture failed to encode: ${encoded.code}: ${encoded.detail}`);
	return new TextDecoder('utf-8', { fatal: true }).decode(encoded.value);
}

function controlTexts(schema, values, name) {
	return values.map((value, index) => controlText(schema, value, `${name}[${index}]`));
}

function controlRequestText(kind, value, name) {
	const encoded = encodeControlRequest(kind, value);
	if (!encoded.ok)
		throw new Error(`${name} control request failed to encode: ${encoded.code}: ${encoded.detail}`);
	return new TextDecoder('utf-8', { fatal: true }).decode(encoded.value);
}

function controlRequestTexts(kind, values, name) {
	return values.map((value, index) => controlRequestText(kind, value, `${name}[${index}]`));
}

function referenceControlResult(bytes, schema, selectInteger) {
	let text;
	try {
		text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
	} catch {
		return { code: 'invalid-utf8', integer: null };
	}

	let value;
	try {
		value = JSON.parse(text);
	} catch {
		return { code: 'invalid-json', integer: null };
	}
	const parsed = schema.safeParse(value);
	if (!parsed.success) return { code: 'invalid-field', integer: null };
	return {
		code: null,
		integer: selectInteger === null ? null : selectInteger(parsed.data),
	};
}

function rawControlCase({
	decoder,
	expectedCode,
	expectedInteger = null,
	name,
	payload,
	referenceNegativeZero = false,
	schema,
	selectInteger = null,
	strictJson = false,
}) {
	const bytes = typeof payload === 'string' ? new TextEncoder().encode(payload) : payload;
	const reference = referenceControlResult(bytes, schema, selectInteger);
	if (referenceNegativeZero && !Object.is(reference.integer, -0)) {
		throw new Error(`${name} reference result must preserve JavaScript negative zero`);
	}
	if (strictJson) {
		if (
			expectedCode !== 'invalid-json' ||
			reference.code === 'invalid-utf8' ||
			reference.code === 'invalid-json'
		) {
			throw new Error(`${name} strict JSON case must extend valid JSON.parse grammar`);
		}
	} else {
		if (reference.code !== expectedCode || reference.integer !== expectedInteger) {
			throw new Error(
				`${name} reference result ${JSON.stringify(reference)} does not match the fixture expectation`,
			);
		}
	}
	return {
		decoder,
		expected_code: expectedCode,
		expected_integer: expectedInteger,
		name,
		payload_base64: Buffer.from(bytes).toString('base64'),
	};
}

function rawControlCases() {
	return [
		rawControlCase({
			decoder: 'empty',
			expectedCode: 'invalid-utf8',
			name: 'invalid-utf8-byte',
			payload: new Uint8Array([0xff]),
			schema: EmptyControlSchema,
		}),
		rawControlCase({
			decoder: 'empty',
			expectedCode: 'invalid-json',
			name: 'malformed-json-grammar',
			payload: '{',
			schema: EmptyControlSchema,
		}),
		rawControlCase({
			decoder: 'empty',
			expectedCode: 'invalid-json',
			name: 'initial-utf8-bom',
			payload: new Uint8Array([0xef, 0xbb, 0xbf, 0x7b, 0x7d]),
			schema: EmptyControlSchema,
			strictJson: true,
		}),
		rawControlCase({
			decoder: 'list-terminals',
			expectedCode: 'invalid-field',
			name: 'numeric-string-does-not-coerce',
			payload: '{"cursor":null,"maximumTerminals":"1"}',
			schema: ListTerminalsSchema,
		}),
		rawControlCase({
			decoder: 'terminal-target',
			expectedCode: 'invalid-field',
			name: 'byte-array-does-not-coerce-to-string',
			payload: JSON.stringify({ terminalId: Array.from({ length: 32 }, () => 49) }),
			schema: TerminalTargetSchema,
		}),
		rawControlCase({
			decoder: 'list-terminals',
			expectedCode: null,
			expectedInteger: 1,
			name: 'binary64-rounds-decimal-to-one',
			payload: '{"cursor":null,"maximumTerminals":1.0000000000000001}',
			schema: ListTerminalsSchema,
			selectInteger: (value) => value.maximumTerminals,
		}),
		rawControlCase({
			decoder: 'set-terminal-defaults',
			expectedCode: null,
			expectedInteger: 0,
			name: 'binary64-negative-underflow-normalizes-to-zero',
			payload: '{"configuration":{"scrollbackBytes":-1e-4000}}',
			referenceNegativeZero: true,
			schema: SetTerminalDefaultsSchema,
			selectInteger: (value) => value.configuration.scrollbackBytes,
		}),
		rawControlCase({
			decoder: 'empty',
			expectedCode: 'invalid-field',
			name: 'unknown-member',
			payload: '{"extra":true}',
			schema: EmptyControlSchema,
		}),
		rawControlCase({
			decoder: 'list-terminals',
			expectedCode: 'invalid-json',
			name: 'exact-duplicate-member',
			payload: '{"cursor":null,"maximumTerminals":1,"maximumTerminals":2}',
			schema: ListTerminalsSchema,
			strictJson: true,
		}),
		rawControlCase({
			decoder: 'list-terminals',
			expectedCode: 'invalid-json',
			name: 'escaped-equivalent-duplicate-member',
			payload: String.raw`{"cursor":null,"maximumTerminals":1,"max\u0069mumTerminals":2}`,
			schema: ListTerminalsSchema,
			strictJson: true,
		}),
		rawControlCase({
			decoder: 'terminal-target',
			expectedCode: 'invalid-json',
			name: 'unpaired-high-surrogate',
			payload: String.raw`{"terminalId":"\ud800"}`,
			schema: TerminalTargetSchema,
			strictJson: true,
		}),
		rawControlCase({
			decoder: 'list-terminals',
			expectedCode: 'invalid-field',
			name: 'unsafe-integer',
			payload: '{"cursor":null,"maximumTerminals":9007199254740992}',
			schema: ListTerminalsSchema,
		}),
		rawControlCase({
			decoder: 'list-terminals',
			expectedCode: 'invalid-field',
			name: 'nonfinite-binary64-result',
			payload: '{"cursor":null,"maximumTerminals":1e4000}',
			schema: ListTerminalsSchema,
		}),
		...['html', 'vt'].map((format) =>
			rawControlCase({
				decoder: 'selection-export',
				expectedCode: 'invalid-field',
				name: `unsupported-selection-export-${format}`,
				payload: JSON.stringify({
					attachmentId: '22'.repeat(16),
					exportSequence: '1',
					format,
				}),
				schema: SelectionExportSchema,
			}),
		),
	];
}

function decodeUtf8BomInput(decoder, payload) {
	switch (decoder) {
		case 'key-input':
			return decodeKeyInput(payload);
		case 'paste-input':
			return decodePasteInput(payload);
		case 'text-input':
			return decodeTextInput(payload);
		default:
			throw new Error(`Unknown UTF-8 BOM input decoder: ${decoder}`);
	}
}

function utf8BomInputCase({ decoder, expectedCode, expectedText = null, name, payload }) {
	const result = decodeUtf8BomInput(decoder, payload);
	if (expectedCode === null) {
		if (!result.ok || result.value.text !== expectedText) {
			throw new Error(`${name} did not preserve its expected text`);
		}
	} else if (result.ok || result.code !== expectedCode) {
		throw new Error(`${name} did not produce ${expectedCode}`);
	}
	return {
		decoder,
		expected_code: expectedCode,
		expected_text_base64:
			expectedText === null ? null : Buffer.from(expectedText, 'utf8').toString('base64'),
		name,
		payload_base64: Buffer.from(payload).toString('base64'),
	};
}

function createUtf8BomConformanceFixture() {
	const attachmentId = '22'.repeat(16);
	const modifiers = {
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
	const key = encodeOrThrow(
		encodeInputRequest(MessageKind.keyInput, {
			action: 'press',
			attachmentId,
			capabilityRevision: '7',
			composing: false,
			consumedModifiers: modifiers,
			key: 'F1',
			modifiers,
			optionAsAlt: false,
			text: '',
			unshiftedCodepoint: 0,
		}),
		'UTF-8 BOM key input',
	);
	const bom = new Uint8Array([0xef, 0xbb, 0xbf]);
	const bomKey = new Uint8Array(key.byteLength + bom.byteLength);
	const keyBody = key.subarray(KEY_INPUT_HEADER_BYTES);
	bomKey.set(key.subarray(0, KEY_INPUT_HEADER_BYTES));
	new DataView(bomKey.buffer).setUint16(30, bom.byteLength + keyBody.byteLength, true);
	bomKey.set(bom, KEY_INPUT_HEADER_BYTES);
	bomKey.set(keyBody, KEY_INPUT_HEADER_BYTES + bom.byteLength);
	const textValue = { attachmentId, capabilityRevision: '7', text: '\ufeff' };
	const text = encodeOrThrow(
		encodeInputRequest(MessageKind.textInput, textValue),
		'UTF-8 BOM text input',
	);
	const paste = encodeOrThrow(
		encodeInputRequest(MessageKind.pasteInput, { ...textValue, bracketed: false }),
		'UTF-8 BOM paste input',
	);

	return {
		cases: [
			utf8BomInputCase({
				decoder: 'key-input',
				expectedCode: 'invalid-field',
				name: 'bom-prefixed-f1-key',
				payload: bomKey,
			}),
			utf8BomInputCase({
				decoder: 'text-input',
				expectedCode: null,
				expectedText: textValue.text,
				name: 'leading-ufeff-text',
				payload: text,
			}),
			utf8BomInputCase({
				decoder: 'paste-input',
				expectedCode: null,
				expectedText: textValue.text,
				name: 'leading-ufeff-paste',
				payload: paste,
			}),
		],
	};
}

function createControlConformanceFixture() {
	const attachmentId = '22'.repeat(16);
	const creationId = '33'.repeat(16);
	const themeOperationId = '66'.repeat(16);
	const otherThemeOperationId = '77'.repeat(16);
	const terminalId = '44'.repeat(16);
	const hostEpoch = '11'.repeat(16);
	const clientId = '55'.repeat(16);
	const protocol = CURRENT_PROTOCOL_VERSION;
	const terminalProfile = {
		colorScheme: 'dark',
		defaultBackgroundRgb: 0x10_10_10,
		defaultCursorBlinking: true,
		defaultCursorRgb: null,
		defaultCursorShape: 'underline',
		defaultForegroundRgb: 0xf0_f0_f0,
		deviceAttributes: {
			primaryConformanceLevel: 65,
			primaryFeatures: [1, 4],
			secondaryDeviceType: 1,
			secondaryFirmwareVersion: 2,
			secondaryRomCartridge: 0,
			tertiaryUnitId: 42,
		},
		enquiryResponse: '\u001b[?1;2c',
		palette: Array.from({ length: 256 }, (_, index) => index * 0x01_01_01),
		versionResponse: '\u001b[>1;2;3c',
	};
	const terminalTheme = {
		colorScheme: terminalProfile.colorScheme,
		defaultBackgroundRgb: terminalProfile.defaultBackgroundRgb,
		defaultCursorRgb: terminalProfile.defaultCursorRgb,
		defaultForegroundRgb: terminalProfile.defaultForegroundRgb,
		palette: terminalProfile.palette,
	};
	const baselineTheme = { operationId: null, revision: '0', terminal: terminalTheme };
	const launch = {
		arguments: ['--login', '项目'],
		cellPixelHeight: 16,
		cellPixelWidth: 8,
		columns: 120,
		configuration: null,
		currentDirectory: '/tmp/项目',
		environment: [
			{ name: 'EMPTY', value: '' },
			{ name: 'REMOVE_ME', value: null },
		],
		executable: '/bin/sh',
		rows: 40,
		terminalProfile,
	};
	const deferredLaunch = {
		...launch,
		cellPixelHeight: null,
		cellPixelWidth: null,
		columns: null,
		rows: null,
	};
	const attachment = { attachmentId, generation: '3', revision: '42' };
	const point = { column: 4, row: '100' };
	const gestureAttachment = { attachmentId, generation: '3' };
	const viewportPoint = { column: 4, row: 10 };
	const failure = (kind, stage) => ({ kind, stage });
	const processObservation = (index) => {
		if (index === 1) {
			return {
				failure: failure('process-observation', 'observe-process'),
				kind: 'unavailable',
			};
		}
		switch (index % 3) {
			case 0:
				return { kind: 'observed', result: { code: 1, kind: 'exit-code' } };
			case 1:
				return { kind: 'observed', result: { kind: 'signal', signal: 15 } };
			default:
				return { kind: 'observed', result: { kind: 'unknown', value: 7 } };
		}
	};
	const identity = { creationId, hostEpoch, terminalId };
	const summary = {
		columns: 120,
		configuration: { scrollbackBytes: 10_000_000 },
		configurationRevision: '2',
		identity,
		revision: '42',
		rows: 40,
		theme: baselineTheme,
	};
	const failurePairs = [
		['process-control', 'dispatch-termination'],
		['process-observation', 'observe-process'],
		['pty-read', 'read-output'],
		['pty-write', 'write-input'],
		['resize', 'apply-resize'],
		['terminal-reply', 'admit-terminal-reply'],
		['worker-integrity', 'synchronize-worker'],
	];
	const commands = {
		apply_terminal_theme: controlRequestText(
			MessageKind.applyTerminalTheme,
			{
				expectedThemeRevision: '0',
				operationId: themeOperationId,
				terminal: terminalTheme,
				terminalId,
			},
			'ApplyTerminalTheme',
		),
		attach_terminal: controlRequestText(
			MessageKind.attachTerminal,
			{
				attachmentId,
				capabilities: { input: true, read: true, resize: false },
				requestResizeLease: false,
				terminalId,
			},
			'AttachTerminal',
		),
		claim_resize_lease: controlRequestText(
			MessageKind.claimResizeLease,
			ClaimResizeLeaseSchema.parse({ attachmentId }),
			'ClaimResizeLease',
		),
		update_attachment_input_capability: controlRequestText(
			MessageKind.updateAttachmentInputCapability,
			UpdateAttachmentInputCapabilitySchema.parse({
				attachmentId,
				capabilityRevision: '2',
				input: false,
			}),
			'UpdateAttachmentInputCapability',
		),
		clear_terminal: controlRequestText(
			MessageKind.clearTerminal,
			{ attachmentId, preservePrompt: false },
			'ClearTerminal',
		),
		create_terminal: controlRequestText(
			MessageKind.createTerminal,
			{
				creationId,
				environmentOverlay: [
					{ name: 'ORCHBEE_AGENT_OBSERVATION_SOCKET', value: '/tmp/orchbee-agent.sock' },
				],
				launch,
			},
			'CreateTerminal',
		),
		create_terminal_deferred: controlRequestText(
			MessageKind.createTerminal,
			{ creationId, environmentOverlay: [], launch: deferredLaunch },
			'CreateTerminal',
		),
		detach_terminal: controlRequestText(
			MessageKind.detachTerminal,
			{ attachmentId },
			'DetachTerminal',
		),
		empty: controlRequestText(MessageKind.ping, {}, 'Ping'),
		frame_applied: controlRequestText(
			MessageKind.frameApplied,
			{ ...attachment, sequence: '7' },
			'FrameApplied',
		),
		hello: controlRequestText(
			MessageKind.hello,
			{
				clientId,
				frameBytes: 8 * 1024 * 1024,
				inputBytes: 1024 * 1024,
				protocol,
				rowWindowBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
				supportedOptionalFeatures: ['future-x'],
			},
			'Hello',
		),
		history_demand: controlRequestTexts(
			MessageKind.setHistoryDemand,
			['following-bottom', 'viewing-history'].map((demand) => ({ attachmentId, demand })),
			'SetHistoryDemand',
		),
		history_observed: controlRequestText(
			MessageKind.historyObserved,
			{ ...attachment, sequence: '5' },
			'HistoryObserved',
		),
		read_process_info: controlRequestText(
			MessageKind.readProcessInfo,
			{ attachmentId },
			'ReadProcessInfo',
		),
		read_process_info_launch_kind: controlRequestText(
			MessageKind.readProcessInfo,
			{ attachmentId, includeLaunchKind: true },
			'ReadProcessInfo with launch kind',
		),

		list_terminals: controlRequestText(
			MessageKind.listTerminals,
			{
				cursor: { inventoryGeneration: '9', offset: '2' },
				maximumTerminals: 64,
			},
			'ListTerminals',
		),
		need_snapshot: controlRequestTexts(
			MessageKind.needSnapshot,
			[
				'base-mismatch',
				'decoder-rejected',
				'dimensions-mismatch',
				'generation-mismatch',
				'identity-mismatch',
				'sequence-gap',
				'store-reset',
			].map((reason, index) => ({ attachmentId, reason, sequence: String(index + 1) })),
			'NeedSnapshot',
		),
		projection_demand: controlRequestTexts(
			MessageKind.setProjectionDemand,
			['active', 'paused'].map((demand) => ({ attachmentId, demand })),
			'SetProjectionDemand',
		),
		read_rows: controlRequestTexts(
			MessageKind.readRows,
			[
				{
					detail: 'text',
					origin: { kind: 'absolute', row: '10' },
					purpose: 'buffer',
					revisionPolicy: 'exact',
				},
				{
					detail: 'render',
					origin: { kind: 'oldest', offset: 2 },
					purpose: 'search',
					revisionPolicy: 'exact',
				},
				{
					detail: 'text',
					origin: { delta: -1, kind: 'committed' },
					purpose: 'viewport',
					revisionPolicy: 'current-view',
				},
				{
					detail: 'render',
					origin: { kind: 'tail', rowsBefore: 3 },
					purpose: 'buffer',
					revisionPolicy: 'exact',
				},
			].map((request, index) => ({
				...attachment,
				...request,
				maximumBytes: 65_536,
				requestSequence: String(index + 1),
				rowCount: 20,
			})),
			'ReadRows',
		),
		resize_terminal: controlRequestText(
			MessageKind.resizeTerminal,
			{
				attachmentId,
				cellPixelHeight: 16,
				cellPixelWidth: 8,
				columns: 100,
				rows: 30,
			},
			'ResizeTerminal',
		),
		reset_terminal: controlRequestText(
			MessageKind.resetTerminal,
			{ attachmentId },
			'ResetTerminal',
		),
		rows_applied: controlRequestText(
			MessageKind.rowsApplied,
			{ ...attachment, requestSequence: '8' },
			'RowsApplied',
		),
		selection_command: controlRequestTexts(
			MessageKind.selectionCommand,
			[
				{ ...attachment, command: 'clear' },
				{ ...attachment, command: 'select-all' },
				{
					...attachment,
					command: 'set-range',
					end: { column: 9, row: '43' },
					rectangle: true,
					start: point,
				},
				{ ...attachment, command: 'select-output', point },
			],
			'SelectionCommand',
		),
		selection_export: controlRequestTexts(
			MessageKind.selectionExport,
			[{ attachmentId, exportSequence: '1', format: 'plain' }],
			'SelectionExport',
		),
		selection_export_cancel: controlRequestTexts(
			MessageKind.selectionExportCancel,
			['clipboard-failed', 'disposed', 'replaced', 'user'].map((reason, index) => ({
				attachmentId,
				exportSequence: String(index + 1),
				reason,
			})),
			'SelectionExportCancel',
		),
		selection_export_chunk_applied: controlRequestText(
			MessageKind.selectionExportChunkApplied,
			{ attachmentId, chunkSequence: 2, exportSequence: '9' },
			'SelectionExportChunkApplied',
		),
		selection_gesture: controlRequestTexts(
			MessageKind.selectionGesture,
			[
				{
					action: 'start',
					...gestureAttachment,
					clickCount: 2,
					point: viewportPoint,
					position: { x: 36, y: 20 },
					selectOutput: false,
				},
				{
					action: 'update',
					...gestureAttachment,
					geometry: { cellWidth: 8, columns: 120, paddingLeft: 0, screenHeight: 640 },
					point: viewportPoint,
					position: { x: 44, y: -1 },
					rectangle: true,
				},
				{ action: 'end', ...gestureAttachment, point: viewportPoint },
				{ action: 'cancel', ...gestureAttachment },
			],
			'SelectionGesture',
		),
		set_terminal_defaults: controlRequestText(
			MessageKind.setTerminalDefaults,
			{ configuration: { scrollbackBytes: 25_000_000 } },
			'SetTerminalDefaults',
		),
		start_terminal: controlRequestText(
			MessageKind.startTerminal,
			{
				geometry: { cellPixelHeight: 18, cellPixelWidth: 9, columns: 96, rows: 32 },
				terminalId,
			},
			'StartTerminal',
		),
		terminal_target: controlRequestText(
			MessageKind.terminateTerminal,
			{ terminalId },
			'TerminalTarget',
		),
		update_terminal_configuration: controlRequestText(
			MessageKind.updateTerminalConfiguration,
			{ configuration: { scrollbackBytes: 0 }, terminalId },
			'UpdateTerminalConfiguration',
		),
	};

	const terminalSummaries = [
		{
			...summary,
			columns: null,
			exit: null,
			failure: null,
			failureContainment: null,
			phase: 'awaiting-start',
			process: null,
			revision: '0',
			rows: null,
			terminateRequested: false,
		},
		{
			...summary,
			exit: null,
			failure: null,
			failureContainment: null,
			phase: 'running',
			process: null,
			terminateRequested: false,
		},
		{
			...summary,
			exit: { kind: 'unknown', value: 7 },
			failure: null,
			failureContainment: null,
			phase: 'finishing',
			process: null,
			terminateRequested: true,
		},
		{
			...summary,
			exit: null,
			failure: failure('pty-read', 'read-output'),
			failureContainment: { kind: 'stopping', process: null },
			phase: 'containing-failure',
			process: null,
			terminateRequested: true,
		},
		{
			...summary,
			exit: null,
			failure: failure('resize', 'apply-resize'),
			failureContainment: {
				cleanupFailure: failure('pty-read', 'release-output'),
				kind: 'blocked',
				process: { kind: 'observed', result: { code: 1, kind: 'exit-code' } },
			},
			phase: 'containing-failure',
			process: null,
			terminateRequested: true,
		},
		{
			...summary,
			exit: { kind: 'signal', signal: 15 },
			failure: null,
			failureContainment: null,
			phase: 'exited',
			process: null,
			terminateRequested: true,
		},
		{
			...summary,
			exit: null,
			failure: failure('worker-integrity', 'synchronize-worker'),
			failureContainment: null,
			phase: 'failed',
			process: {
				failure: failure('process-observation', 'observe-process'),
				kind: 'unavailable',
			},
			terminateRequested: true,
		},
	];
	const outputs = {
		attachment_acknowledged: controlText(
			AttachmentAcknowledgedSchema,
			{ attachmentId },
			'AttachmentAcknowledged',
		),
		attachment_capabilities: controlText(
			AttachmentCapabilitiesSchema,
			{
				attachmentId,
				capabilities: { input: false, read: false, resize: false },
				capabilityRevision: '2',
				resizeLease: false,
				terminalId,
			},
			'AttachmentCapabilities',
		),
		resize_lease_changed: controlText(
			ResizeLeaseChangedSchema,
			{ attachmentId, resizeLease: true, terminalId },
			'ResizeLeaseChanged',
		),
		empty: controlText(EmptyControlSchema, {}, 'EmptyControl'),
		frame_applied: commands.frame_applied,
		hello_accepted: controlText(
			HelloAcceptedSchema,
			{
				frameBytes: 8 * 1024 * 1024,
				hostEpoch,
				hostVersion: '0.1.0',
				inputBytes: 1024 * 1024,
				protocol,
				rowWindowBytes: PROTOCOL_LIMITS.maxRowWindowBytes,
				supportedOptionalFeatures: [],
			},
			'HelloAccepted',
		),
		history_changed: controlTexts(
			HistoryChangedSchema,
			['evicted', 'none', 'valid'].map((anchorStatus, index) => ({
				anchorStatus,
				...attachment,
				sequence: String(index + 1),
				totalRows: '125',
			})),
			'HistoryChanged',
		),
		history_demand_accepted: controlTexts(
			HistoryDemandAcceptedSchema,
			['following-bottom', 'viewing-history'].map((demand) => ({ attachmentId, demand })),
			'HistoryDemandAccepted',
		),
		history_observed: commands.history_observed,
		input_accepted: controlText(
			InputAcceptedSchema,
			{ acceptedBytes: 12, attachmentId, capabilityRevision: '1' },
			'InputAccepted',
		),
		need_snapshot: commands.need_snapshot,
		projection_blocked: controlTexts(
			ProjectionBlockedSchema,
			['capture-allocation-failed', 'projection-too-large'].map((reason) => ({
				attachmentId,
				reason,
				revision: '42',
			})),
			'ProjectionBlocked',
		),
		projection_demand_accepted: controlTexts(
			ProjectionDemandAcceptedSchema,
			['active', 'paused'].map((demand) => ({ attachmentId, demand })),
			'ProjectionDemandAccepted',
		),
		protocol_rejected: controlText(
			ProtocolRejectedSchema,
			{ client: { major: 2, minor: 0, patch: 0 }, host: protocol, reason: 'version-mismatch' },
			'ProtocolRejected',
		),
		resize_accepted: controlText(
			ResizeAcceptedSchema,
			{
				attachmentId,
				cellPixelHeight: 16,
				cellPixelWidth: 8,
				columns: 100,
				revision: '43',
				rows: 30,
			},
			'ResizeAccepted',
		),
		rows_applied: commands.rows_applied,
		selection_changed: controlTexts(
			SelectionChangedSchema,
			[
				{ attachmentId, state: 'none' },
				{ attachmentId, state: 'evicted' },
				{
					attachmentId,
					end: { column: 7, row: '102' },
					preview: null,
					rectangle: true,
					start: point,
					state: 'active',
				},
			],
			'SelectionChanged',
		),
		selection_export_chunk_applied: commands.selection_export_chunk_applied,
		selection_export_finished: controlTexts(
			SelectionExportFinishedSchema,
			['cancelled', 'completed'].map((status) => ({
				attachmentId,
				byteLength: 12,
				exportSequence: '9',
				status,
			})),
			'SelectionExportFinished',
		),
		selection_export_started: controlTexts(
			SelectionExportStartedSchema,
			['plain'].map((format, index) => ({
				attachmentId,
				byteLength: 10 + index,
				exportSequence: String(index + 1),
				format,
			})),
			'SelectionExportStarted',
		),
		terminal_acknowledged: controlText(
			TerminalAcknowledgedSchema,
			{ terminalId },
			'TerminalAcknowledged',
		),
		terminal_application_signal: controlText(
			TerminalApplicationSignalSchema,
			{
				channel: 'orchbee.agent.v1',
				identity,
				payload: JSON.stringify({ activity: 'command', version: 1 }),
				sequence: '8',
			},
			'TerminalApplicationSignal',
		),
		terminal_bell: controlText(TerminalBellSchema, { attachmentId, sequence: '6' }, 'TerminalBell'),
		terminal_clipboard_write_request: controlText(
			TerminalClipboardWriteRequestSchema,
			{ attachmentId, sequence: '7', text: 'copied from terminal' },
			'TerminalClipboardWriteRequest',
		),
		terminal_configuration_accepted: controlText(
			TerminalConfigurationAcceptedSchema,
			{
				configuration: { scrollbackBytes: 25_000_000 },
				configurationRevision: '3',
				generation: '4',
				historyRowsEvicted: '5',
				revision: '43',
				terminalId,
			},
			'TerminalConfigurationAccepted',
		),
		terminal_control_accepted: controlTexts(
			TerminalControlAcceptedSchema,
			['clear', 'reset'].map((operation, index) => ({
				attachmentId,
				generation: String(4 + index),
				operation,
				revision: String(43 + index),
				terminalId,
			})),
			'TerminalControlAccepted',
		),
		terminal_defaults_accepted: controlText(
			TerminalDefaultsAcceptedSchema,
			{ configuration: { scrollbackBytes: 25_000_000 }, revision: '3' },
			'TerminalDefaultsAccepted',
		),
		terminal_exited: controlTexts(
			TerminalExitedSchema,
			[
				{ exit: { code: 0, kind: 'exit-code' }, finalRevision: '42', identity },
				{ exit: { kind: 'signal', signal: 15 }, finalRevision: '42', identity },
				{ exit: { kind: 'unknown', value: 7 }, finalRevision: '42', identity },
			],
			'TerminalExited',
		),
		terminal_failed: controlTexts(
			TerminalFailedSchema,
			failurePairs.map(([kind, stage], index) => ({
				failure: failure(kind, stage),
				finalRevision: String(42 + index),
				identity,
				process: processObservation(index),
			})),
			'TerminalFailed',
		),
		terminal_identity: controlText(TerminalIdentitySchema, identity, 'TerminalIdentity'),
		process_info: controlText(
			TerminalProcessInfoSchema,
			{
				attachmentId,
				terminalId,
				kind: 'windows-job',
				rootPid: 41,
				processes: [
					[
						41,
						17,
						'133999999999999999',
						'attached',
						'C:\\Windows\\System32\\cmd.exe',
						'cmd /C echo 中文 😀',
						null,
					],
				],
			},
			'ReadProcessInfo response',
		),
		process_info_launch_kind: controlText(
			TerminalProcessInfoSchema,
			{
				attachmentId,
				terminalId,
				kind: 'windows-job',
				launchKind: 'cmd',
				rootPid: 41,
				processes: [
					[
						41,
						17,
						'133999999999999999',
						'attached',
						'C:\\Windows\\System32\\cmd.exe',
						'cmd.exe',
						null,
					],
				],
			},
			'ReadProcessInfo response with launch kind',
		),
		terminal_list: controlText(
			TerminalListSchema,
			{ inventoryGeneration: '9', nextCursor: null, terminals: terminalSummaries },
			'TerminalList',
		),
		terminal_start_accepted: controlTexts(
			TerminalStartAcceptedSchema,
			['started', 'already-started'].map((transition) => ({
				geometry: { cellPixelHeight: 18, cellPixelWidth: 9, columns: 96, rows: 32 },
				identity,
				transition,
			})),
			'TerminalStartAccepted',
		),
		terminal_started: controlText(TerminalStartedSchema, identity, 'TerminalStarted'),
		terminal_theme_result: controlTexts(
			TerminalThemeResultSchema,
			[
				{
					current: { operationId: themeOperationId, revision: '1', terminal: terminalTheme },
					operationId: themeOperationId,
					terminalId,
					transition: 'applied',
				},
				{
					current: { operationId: themeOperationId, revision: '1', terminal: terminalTheme },
					operationId: themeOperationId,
					terminalId,
					transition: 'already-applied',
				},
				{
					current: {
						operationId: otherThemeOperationId,
						revision: '2',
						terminal: terminalTheme,
					},
					operationId: themeOperationId,
					terminalId,
					transition: 'stale',
				},
			],
			'TerminalThemeResult',
		),
		typed_failure: controlTexts(
			TypedFailureSchema,
			[
				{ code: 'invalid-state', message: 'The command is not legal now.', retryable: false },
				{ code: 'backpressure', message: 'Try again later.', retryable: true },
				{
					code: 'prompt-unconfirmed',
					message: 'the shell is not confirmed at a fresh, empty prompt; nothing was typed',
					retryable: false,
					reason: 'not-at-prompt',
				},
			],
			'TypedFailure',
		),
	};

	return {
		commands,
		outputs,
		raw_cases: rawControlCases(),
	};
}

function locateTables(bytes) {
	const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
	const stringCount = view.getUint32(124, true);
	const styleCount = view.getUint16(128, true);
	let offset = 160;
	for (let index = 0; index < stringCount; index += 1) {
		const length = view.getUint32(offset, true);
		offset += 4 + length;
	}
	return { firstRow: offset + styleCount * 20, firstStyle: offset };
}

function malformedCorpus(frameBytes) {
	const invalidMagic = frameBytes.slice();
	invalidMagic[0] = 0;

	const lengthMismatch = frameBytes.slice();
	new DataView(lengthMismatch.buffer).setUint32(8, lengthMismatch.byteLength - 1, true);

	const invalidUtf8 = frameBytes.slice();
	invalidUtf8[164] = 0xff;

	const rowOutOfBounds = frameBytes.slice();
	new DataView(rowOutOfBounds.buffer).setUint16(locateTables(rowOutOfBounds).firstRow, 2, true);

	const unknownStyleFlag = frameBytes.slice();
	new DataView(unknownStyleFlag.buffer).setUint16(
		locateTables(unknownStyleFlag).firstStyle + 16,
		0x0100,
		true,
	);

	const unknownRequiredExtension = frameBytes.slice();
	new DataView(unknownRequiredExtension.buffer).setUint16(
		unknownRequiredExtension.byteLength - 10,
		0x8000,
		true,
	);

	return [
		{
			bytesBase64: Buffer.from(frameBytes.subarray(0, 120)).toString('base64'),
			code: 'truncated',
			name: 'truncated-header',
		},
		{
			bytesBase64: Buffer.from(invalidMagic).toString('base64'),
			code: 'invalid-magic',
			name: 'invalid-magic',
		},
		{
			bytesBase64: Buffer.from(lengthMismatch).toString('base64'),
			code: 'length-mismatch',
			name: 'length-mismatch',
		},
		{
			bytesBase64: Buffer.from(invalidUtf8).toString('base64'),
			code: 'invalid-utf8',
			name: 'invalid-utf8',
		},
		{
			bytesBase64: Buffer.from(rowOutOfBounds).toString('base64'),
			code: 'invalid-field',
			name: 'row-out-of-bounds',
		},
		{
			bytesBase64: Buffer.from(unknownStyleFlag).toString('base64'),
			code: 'unknown-required-flag',
			name: 'unknown-style-flag',
		},
		{
			bytesBase64: Buffer.from(unknownRequiredExtension).toString('base64'),
			code: 'unknown-required-flag',
			name: 'unknown-required-extension',
		},
	];
}

const ROW_WINDOW_HEADER_BYTES = 112;

function authoredTextRowWindow({ columns, rows, startRow = 100n, totalRows = 125n }) {
	return {
		attachmentId: '22222222222222222222222222222222',
		columns,
		detail: 'text',
		generation: 3n,
		hostEpoch: '11111111111111111111111111111111',
		revision: 42n,
		rows,
		sequence: 8n,
		startRow,
		totalRows,
	};
}

function identityCellMappings(length) {
	return Array.from({ length }, (_, index) => ({
		byteOffset: index,
		column: index,
		utf16Offset: index,
		width: 1,
	}));
}

function encodedTextRowWindow(window, name) {
	return encodeOrThrow(encodeTerminalRowWindow(window), name);
}

function textObservation(rowCount) {
	return {
		detail: 'text',
		ignoredOptionalFlags: 0,
		rowCount,
	};
}

/// Hand-assembles a text-detail row window from the wire layout so the corpus can
/// carry forms the TypeScript encoder legitimately refuses to emit.
function assembleTextRowWindowBytes({
	columns,
	entryCount,
	rows,
	startRow = 100n,
	totalRows = 125n,
	trailingBytes = 0,
}) {
	const rowParts = rows.map((row) => {
		const part = new Uint8Array(12 + row.text.byteLength + row.mappings.length * 12);
		const view = new DataView(part.buffer);
		part[0] = row.flags ?? 0;
		part[1] = row.promptClass ?? 0;
		view.setUint16(2, 0, true);
		view.setUint32(4, row.text.byteLength, true);
		view.setUint32(8, row.mappings.length, true);
		part.set(row.text, 12);
		row.mappings.forEach((mapping, index) => {
			const offset = 12 + row.text.byteLength + index * 12;
			view.setUint32(offset, mapping.byteOffset, true);
			view.setUint32(offset + 4, mapping.utf16Offset, true);
			view.setUint16(offset + 8, mapping.column, true);
			part[offset + 10] = mapping.width;
			part[offset + 11] = 0;
		});
		return part;
	});
	const bodyLength = rowParts.reduce((total, part) => total + part.byteLength, 0) + trailingBytes;
	const bytes = new Uint8Array(ROW_WINDOW_HEADER_BYTES + bodyLength);
	const view = new DataView(bytes.buffer);
	bytes.set([0x4f, 0x42, 0x52, 0x57], 0);
	bytes[4] = TERMINAL_ROW_WINDOW_FORMAT.major;
	bytes[5] = TERMINAL_ROW_WINDOW_FORMAT.minor;
	bytes[6] = 2; // text detail
	bytes[7] = 0;
	view.setUint32(8, bytes.byteLength, true);
	view.setUint16(12, ROW_WINDOW_HEADER_BYTES, true);
	view.setUint16(14, columns, true);
	bytes.fill(0x11, 16, 32);
	bytes.fill(0x22, 32, 48);
	view.setBigUint64(48, 8n, true);
	view.setBigUint64(56, 42n, true);
	view.setBigUint64(64, 3n, true);
	view.setBigUint64(72, totalRows, true);
	view.setBigUint64(80, startRow, true);
	view.setUint16(88, rows.length, true);
	view.setUint16(90, 0, true);
	view.setUint32(92, entryCount, true);
	view.setUint32(96, bodyLength, true);
	bytes[100] = TERMINAL_ROW_WINDOW_FORMAT.patch;
	let offset = ROW_WINDOW_HEADER_BYTES;
	for (const part of rowParts) {
		bytes.set(part, offset);
		offset += part.byteLength;
	}
	return bytes;
}

function rowWindowView(bytes) {
	return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
}

function malformedRowWindowCorpus(rowWindowBytes, renderRowWindowBytes) {
	const invalidMagic = rowWindowBytes.slice();
	invalidMagic[0] = 0;

	const lengthMismatch = rowWindowBytes.slice();
	new DataView(lengthMismatch.buffer).setUint32(8, lengthMismatch.byteLength - 1, true);

	const unknownRequiredFlag = rowWindowBytes.slice();
	unknownRequiredFlag[7] = 0x80;

	const invalidUtf8 = rowWindowBytes.slice();
	invalidUtf8[124] = 0xff;

	const invalidMappingOrder = rowWindowBytes.slice();
	new DataView(invalidMappingOrder.buffer).setUint16(152, 0, true);

	const renderView = new DataView(
		renderRowWindowBytes.buffer,
		renderRowWindowBytes.byteOffset,
		renderRowWindowBytes.byteLength,
	);
	const projectionLength = renderView.getUint32(101, true);
	const graphicsLength = renderView.getUint32(105, true);
	const projectionSectionMismatch = renderRowWindowBytes.slice();
	const projectionSectionView = new DataView(projectionSectionMismatch.buffer);
	projectionSectionView.setUint32(101, projectionLength - 1, true);
	projectionSectionView.setUint32(105, graphicsLength + 1, true);
	const graphicsSectionMismatch = renderRowWindowBytes.slice();
	new DataView(graphicsSectionMismatch.buffer).setUint32(105, graphicsLength - 1, true);
	const graphicsUpdateNotSnapshot = renderRowWindowBytes.slice();
	graphicsUpdateNotSnapshot[112 + projectionLength + 7] = 2;

	const cases = [
		{
			bytesBase64: Buffer.from(rowWindowBytes.subarray(0, 100)).toString('base64'),
			code: 'truncated',
			name: 'truncated-header',
		},
		{
			bytesBase64: Buffer.from(invalidMagic).toString('base64'),
			code: 'invalid-magic',
			name: 'invalid-magic',
		},
		{
			bytesBase64: Buffer.from(lengthMismatch).toString('base64'),
			code: 'length-mismatch',
			name: 'length-mismatch',
		},
		{
			bytesBase64: Buffer.from(unknownRequiredFlag).toString('base64'),
			code: 'unknown-required-flag',
			name: 'unknown-required-flag',
		},
		{
			bytesBase64: Buffer.from(invalidUtf8).toString('base64'),
			code: 'invalid-utf8',
			name: 'invalid-utf8',
		},
		{
			bytesBase64: Buffer.from(invalidMappingOrder).toString('base64'),
			code: 'invalid-field',
			name: 'invalid-mapping-order',
		},
		{
			bytesBase64: Buffer.from(projectionSectionMismatch).toString('base64'),
			code: 'length-mismatch',
			name: 'render-projection-section-length-mismatch',
		},
		{
			bytesBase64: Buffer.from(graphicsSectionMismatch).toString('base64'),
			code: 'length-mismatch',
			name: 'render-graphics-section-length-mismatch',
		},
		{
			bytesBase64: Buffer.from(graphicsUpdateNotSnapshot).toString('base64'),
			code: 'invalid-field',
			name: 'render-graphics-update-not-snapshot',
		},
	];

	const canonicalView = rowWindowView(rowWindowBytes);
	const firstRow = ROW_WINDOW_HEADER_BYTES;
	const firstTextLength = canonicalView.getUint32(firstRow + 4, true);
	const firstMappings = firstRow + 12 + firstTextLength;
	const unsupportedVersion = rowWindowBytes.slice();
	unsupportedVersion[5] = 2;

	const zeroSequence = rowWindowBytes.slice();
	zeroSequence.fill(0, 48, 56);

	const zeroGeneration = rowWindowBytes.slice();
	zeroGeneration.fill(0, 64, 72);

	const zeroHostEpoch = rowWindowBytes.slice();
	zeroHostEpoch.fill(0, 16, 32);

	const windowBeyondTotal = rowWindowBytes.slice();
	rowWindowView(windowBeyondTotal).setBigUint64(80, 124n, true);

	const invalidDetailCode = rowWindowBytes.slice();
	invalidDetailCode[6] = 3;

	const columnsZero = rowWindowBytes.slice();
	rowWindowView(columnsZero).setUint16(14, 0, true);

	const rowCountZero = rowWindowBytes.slice();
	rowWindowView(rowCountZero).setUint16(88, 0, true);

	const entryCountOverCells = rowWindowBytes.slice();
	rowWindowView(entryCountOverCells).setUint32(92, 262145, true);

	const entryCountMismatch = rowWindowBytes.slice();
	rowWindowView(entryCountMismatch).setUint32(92, 5, true);

	const adjacentWrapDisagree = rowWindowBytes.slice();
	adjacentWrapDisagree[firstRow] &= ~0x01;

	const promptClassRange = rowWindowBytes.slice();
	promptClassRange[firstRow + 1] = 3;

	const rowReservedFlag = rowWindowBytes.slice();
	rowWindowView(rowReservedFlag).setUint16(firstRow + 2, 1, true);

	const rowFlagUnknown = rowWindowBytes.slice();
	rowFlagUnknown[firstRow] |= 0x08;

	const mappingReservedFlag = rowWindowBytes.slice();
	mappingReservedFlag[firstMappings + 11] = 1;

	const mappingFirstNonzero = rowWindowBytes.slice();
	rowWindowView(mappingFirstNonzero).setUint32(firstMappings, 1, true);

	const mappingMidScalar = rowWindowBytes.slice();
	rowWindowView(mappingMidScalar).setUint32(firstMappings + 12, 2, true);

	const mappingUtf16Mismatch = rowWindowBytes.slice();
	rowWindowView(mappingUtf16Mismatch).setUint32(firstMappings + 28, 3, true);

	const mappingWidthInvalid = rowWindowBytes.slice();
	mappingWidthInvalid[firstMappings + 10] = 0;

	const mappingColumnOverflow = rowWindowBytes.slice();
	rowWindowView(mappingColumnOverflow).setUint16(firstMappings + 44, 6, true);

	const mappingByteOrder = rowWindowBytes.slice();
	rowWindowView(mappingByteOrder).setUint32(firstMappings + 12, 0, true);

	const identityAsciiBytes = encodedTextRowWindow(
		authoredTextRowWindow({
			columns: 4,
			rows: [
				{
					index: 100n,
					mappings: identityCellMappings(4),
					promptClass: 'none',
					text: 'ab\u{20}\u{20}',
					wrap: false,
					wrapContinuation: false,
				},
			],
		}),
		'authored identity-ascii row window',
	);

	const identityNonascii = identityAsciiBytes.slice();
	identityNonascii[124] = 0xc3;
	identityNonascii[125] = 0xa9;

	const identityWithMappings = identityAsciiBytes.slice();
	rowWindowView(identityWithMappings).setUint32(firstRow + 8, 1, true);

	const identityOverColumns = identityAsciiBytes.slice();
	rowWindowView(identityOverColumns).setUint16(14, 3, true);

	const nonemptyWithoutMapping = identityAsciiBytes.slice();
	nonemptyWithoutMapping[firstRow] &= ~0x04;

	const mappingCountOverColumns = identityAsciiBytes.slice();
	rowWindowView(mappingCountOverColumns).setUint32(firstRow + 8, 5, true);

	const overdeclaredTextLength = identityAsciiBytes.slice();
	rowWindowView(overdeclaredTextLength).setUint32(firstRow + 4, 0xffff_ffff, true);

	const cellSpanOverLimit = assembleTextRowWindowBytes({
		columns: 6,
		entryCount: 3,
		rows: [
			{
				mappings: [
					{ byteOffset: 0, column: 0, utf16Offset: 0, width: 1 },
					{ byteOffset: 16385, column: 1, utf16Offset: 16385, width: 1 },
					{ byteOffset: 32768, column: 2, utf16Offset: 32768, width: 1 },
				],
				text: new Uint8Array(32769).fill(0x61),
			},
		],
	});

	const trailingSpanOverLimit = assembleTextRowWindowBytes({
		columns: 6,
		entryCount: 2,
		rows: [
			{
				mappings: [
					{ byteOffset: 0, column: 0, utf16Offset: 0, width: 1 },
					{ byteOffset: 1, column: 1, utf16Offset: 1, width: 1 },
				],
				text: new Uint8Array(16386).fill(0x61),
			},
		],
	});

	const trailingGarbage = assembleTextRowWindowBytes({
		columns: 6,
		entryCount: 1,
		rows: [
			{
				mappings: [{ byteOffset: 0, column: 0, utf16Offset: 0, width: 1 }],
				text: new Uint8Array([0x61]),
			},
		],
		trailingBytes: 1,
	});

	const logicalEntriesOverLimit = assembleTextRowWindowBytes({
		columns: 4096,
		entryCount: 262144,
		rows: Array.from({ length: 65 }, () => ({
			flags: 0x04,
			mappings: [],
			text: new Uint8Array(4096).fill(0x61),
		})),
		totalRows: 200n,
	});

	cases.push(
		{
			bytesBase64: Buffer.from(unsupportedVersion).toString('base64'),
			code: 'unsupported-version',
			name: 'obrw-text-unsupported-version',
		},
		{
			bytesBase64: Buffer.from(zeroSequence).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-zero-sequence',
		},
		{
			bytesBase64: Buffer.from(zeroGeneration).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-zero-generation',
		},
		{
			bytesBase64: Buffer.from(zeroHostEpoch).toString('base64'),
			code: 'invalid-identifier',
			name: 'obrw-text-zero-host-epoch',
		},
		{
			bytesBase64: Buffer.from(windowBeyondTotal).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-window-beyond-total',
		},
		{
			bytesBase64: Buffer.from(invalidDetailCode).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-invalid-detail-code',
		},
		{
			bytesBase64: Buffer.from(columnsZero).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-columns-zero',
		},
		{
			bytesBase64: Buffer.from(rowCountZero).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-row-count-zero',
		},
		{
			bytesBase64: Buffer.from(entryCountOverCells).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-entry-count-over-cells',
		},
		{
			bytesBase64: Buffer.from(entryCountMismatch).toString('base64'),
			code: 'length-mismatch',
			name: 'obrw-text-entry-count-mismatch',
		},
		{
			bytesBase64: Buffer.from(adjacentWrapDisagree).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-adjacent-wrap-disagree',
		},
		{
			bytesBase64: Buffer.from(promptClassRange).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-prompt-class-range',
		},
		{
			bytesBase64: Buffer.from(rowReservedFlag).toString('base64'),
			code: 'unknown-required-flag',
			name: 'obrw-text-row-reserved-flag',
		},
		{
			bytesBase64: Buffer.from(rowFlagUnknown).toString('base64'),
			code: 'unknown-required-flag',
			name: 'obrw-text-row-flag-unknown',
		},
		{
			bytesBase64: Buffer.from(mappingReservedFlag).toString('base64'),
			code: 'unknown-required-flag',
			name: 'obrw-text-mapping-reserved-flag',
		},
		{
			bytesBase64: Buffer.from(mappingFirstNonzero).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-mapping-first-nonzero',
		},
		{
			bytesBase64: Buffer.from(mappingMidScalar).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-mapping-mid-scalar',
		},
		{
			bytesBase64: Buffer.from(mappingUtf16Mismatch).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-mapping-utf16-mismatch',
		},
		{
			bytesBase64: Buffer.from(mappingWidthInvalid).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-mapping-width-invalid',
		},
		{
			bytesBase64: Buffer.from(mappingColumnOverflow).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-mapping-column-overflow',
		},
		{
			bytesBase64: Buffer.from(mappingByteOrder).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-mapping-byte-order',
		},
		{
			bytesBase64: Buffer.from(identityNonascii).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-identity-nonascii',
		},
		{
			bytesBase64: Buffer.from(identityWithMappings).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-identity-with-mappings',
		},
		{
			bytesBase64: Buffer.from(identityOverColumns).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-identity-over-columns',
		},
		{
			bytesBase64: Buffer.from(nonemptyWithoutMapping).toString('base64'),
			code: 'invalid-field',
			name: 'obrw-text-nonempty-without-mapping',
		},
		{
			bytesBase64: Buffer.from(mappingCountOverColumns).toString('base64'),
			code: 'limit-exceeded',
			name: 'obrw-text-mapping-count-over-columns',
		},
		{
			bytesBase64: Buffer.from(overdeclaredTextLength).toString('base64'),
			code: 'truncated',
			name: 'obrw-text-overdeclared-text-length',
		},
		{
			bytesBase64: Buffer.from(cellSpanOverLimit).toString('base64'),
			code: 'limit-exceeded',
			name: 'obrw-text-cell-span-over-16k',
		},
		{
			bytesBase64: Buffer.from(trailingSpanOverLimit).toString('base64'),
			code: 'limit-exceeded',
			name: 'obrw-text-trailing-span-over-16k',
		},
		{
			bytesBase64: Buffer.from(trailingGarbage).toString('base64'),
			code: 'length-mismatch',
			name: 'obrw-text-trailing-garbage',
		},
		{
			bytesBase64: Buffer.from(logicalEntriesOverLimit).toString('base64'),
			code: 'limit-exceeded',
			name: 'obrw-text-logical-entries-over-limit',
		},
	);
	return cases;
}

function projectionSectionOffset(bytes, section) {
	return new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength).getUint32(
		160 + section * 8,
		true,
	);
}

function mutatedBytes(source, offset, replacement) {
	const bytes = source.slice();
	bytes.set(replacement, offset);
	return bytes;
}

function packedProjectionFailureCase({ bytes, code, codec, name, strictBoundary }) {
	return {
		bytesBase64: Buffer.from(bytes).toString('base64'),
		codec,
		expected: { code, result: 'failure' },
		name,
		strictBoundary,
	};
}

function packedProjectionSuccessCase({ bytes, codec, name, observations, strictBoundary }) {
	return {
		bytesBase64: Buffer.from(bytes).toString('base64'),
		codec,
		expected: { observations, result: 'success' },
		name,
		strictBoundary,
	};
}

function packedProjectionConformanceCorpus(
	projectionBytes,
	rowWindowBytes,
	renderRowWindowBytes,
	canonicalFrame,
) {
	const width = projectionSectionOffset(projectionBytes, TerminalProjectionSection.widths);
	const rowFlags = projectionSectionOffset(projectionBytes, TerminalProjectionSection.rowFlags);
	const rowIndices = projectionSectionOffset(projectionBytes, TerminalProjectionSection.rowIndices);
	const prompt = projectionSectionOffset(
		projectionBytes,
		TerminalProjectionSection.rowPromptClasses,
	);
	const stringSection = projectionSectionOffset(
		projectionBytes,
		TerminalProjectionSection.stringBytes,
	);
	const graphemeLengths = projectionSectionOffset(
		projectionBytes,
		TerminalProjectionSection.graphemeLengths,
	);
	const graphemeOffsets = projectionSectionOffset(
		projectionBytes,
		TerminalProjectionSection.graphemeOffsets,
	);
	const foreground = projectionSectionOffset(
		projectionBytes,
		TerminalProjectionSection.foregroundColors,
	);
	const styleFlags = projectionSectionOffset(projectionBytes, TerminalProjectionSection.styleFlags);
	const underlineKinds = projectionSectionOffset(
		projectionBytes,
		TerminalProjectionSection.underlineKinds,
	);
	const semantic = projectionSectionOffset(
		projectionBytes,
		TerminalProjectionSection.semanticContents,
	);
	const protectedCells = projectionSectionOffset(
		projectionBytes,
		TerminalProjectionSection.protectedCells,
	);
	const projectionMutations = [
		{
			bytes: [0xff],
			code: 'invalid-field',
			name: 'obpb-width-out-of-range',
			offset: width,
		},
		{
			bytes: [0x05],
			code: 'unknown-required-flag',
			name: 'obpb-row-flag-unknown',
			offset: rowFlags,
		},
		{
			bytes: [0x02, 0x00],
			code: 'invalid-field',
			name: 'obpb-row-index-out-of-range',
			offset: rowIndices + 2,
		},
		{
			bytes: [0x03],
			code: 'invalid-field',
			name: 'obpb-prompt-class-out-of-range',
			offset: prompt,
		},
		{
			bytes: [0xff],
			code: 'invalid-utf8',
			name: 'obpb-whole-string-invalid-utf8',
			offset: stringSection + 25,
		},
		{
			bytes: [0x02, 0x00],
			code: 'invalid-utf8',
			name: 'obpb-grapheme-length-splits-scalar',
			offset: graphemeLengths,
		},
		{
			bytes: [0xff, 0xff, 0xff, 0xff],
			code: 'invalid-field',
			name: 'obpb-grapheme-offset-null-nonnullable',
			offset: graphemeOffsets,
		},
		{
			bytes: [0x00, 0x00, 0x00, 0x04],
			code: 'invalid-field',
			name: 'obpb-color-out-of-range',
			offset: foreground,
		},
		{
			bytes: [0x10, 0x00, 0x00, 0x03],
			code: 'invalid-field',
			name: 'obpb-palette-index-out-of-range',
			offset: foreground,
		},
		{
			bytes: [0x01, 0x01, 0x00, 0x03],
			code: 'invalid-field',
			name: 'obpb-palette-reserved-bits',
			offset: foreground,
		},
		{
			bytes: [0x00, 0x01],
			code: 'unknown-required-flag',
			name: 'obpb-style-flag-unknown',
			offset: styleFlags,
		},
		{
			bytes: [0x06],
			code: 'invalid-field',
			name: 'obpb-underline-kind-out-of-range',
			offset: underlineKinds,
		},
		{
			bytes: [0x02],
			code: 'invalid-field',
			name: 'obpb-width-spacer-tail-without-head',
			offset: width,
		},
		{
			bytes: [0x01],
			code: 'invalid-field',
			name: 'obpb-row-ends-with-wide-cell',
			offset: width + 5,
		},
		{
			bytes: [0x03],
			code: 'invalid-field',
			name: 'obpb-semantic-out-of-range',
			offset: semantic,
		},
		{
			bytes: [0x02],
			code: 'invalid-field',
			name: 'obpb-protected-out-of-range',
			offset: protectedCells,
		},
		{
			bytes: [0x00],
			code: 'invalid-field',
			name: 'obpb-adjacent-wrap-disagreement',
			offset: rowFlags + 1,
		},
		{
			bytes: [0x8d, 0x62, 0x08, 0x00],
			code: 'unknown-required-flag',
			name: 'obpb-interaction-unknown-required-flag',
			offset: 108,
		},
		{
			bytes: [0x9d, 0x62, 0x00, 0x00],
			code: 'invalid-field',
			name: 'obpb-interaction-mouse-mode-conflict',
			offset: 108,
		},
	];
	const cases = projectionMutations.map((mutation) =>
		packedProjectionFailureCase({
			bytes: mutatedBytes(projectionBytes, mutation.offset, mutation.bytes),
			code: mutation.code,
			codec: 'OBPB',
			name: mutation.name,
			strictBoundary: 'projection-buffer',
		}),
	);

	const sparseFrame = frameFromJson(canonicalFrame);
	if (sparseFrame.rowData.length !== 2) {
		throw new Error('Sparse projection conformance source must contain exactly two rows');
	}
	const sparseProjectionBytes = encodeOrThrow(
		encodeTerminalProjectionBuffer({
			...sparseFrame,
			extensions: [],
			rowData: sparseFrame.rowData.map((row, index) => ({
				...row,
				index: index === 0 ? 0 : 2,
			})),
			rows: 3,
		}),
		'sparse terminal projection buffer',
	);
	const sparseRowIndices = projectionSectionOffset(
		sparseProjectionBytes,
		TerminalProjectionSection.rowIndices,
	);
	cases.push(
		packedProjectionSuccessCase({
			bytes: sparseProjectionBytes,
			codec: 'OBPB',
			name: 'obpb-sparse-delta-increasing-row-indices',
			observations: {
				ignoredOptionalFlags: 0,
				kind: 'delta',
				rowCount: 2,
				rowIndices: [0, 2],
			},
			strictBoundary: 'projection-buffer',
		}),
		packedProjectionFailureCase({
			bytes: mutatedBytes(sparseProjectionBytes, sparseRowIndices + 2, [0x00, 0x00]),
			code: 'invalid-field',
			codec: 'OBPB',
			name: 'obpb-sparse-delta-duplicate-row-indices',
			strictBoundary: 'projection-buffer',
		}),
		packedProjectionFailureCase({
			bytes: mutatedBytes(sparseProjectionBytes, sparseRowIndices, [0x02, 0x00, 0x00, 0x00]),
			code: 'invalid-field',
			codec: 'OBPB',
			name: 'obpb-sparse-delta-descending-row-indices',
			strictBoundary: 'projection-buffer',
		}),
	);

	for (const [flag, name] of [
		[0x01, 'obpb-optional-header-flag-01'],
		[0x7f, 'obpb-optional-header-flags-7f'],
	]) {
		cases.push(
			packedProjectionSuccessCase({
				bytes: mutatedBytes(projectionBytes, 7, [flag]),
				codec: 'OBPB',
				name,
				observations: {
					ignoredOptionalFlags: flag,
					kind: 'delta',
					rowCount: 2,
					rowIndices: [0, 1],
				},
				strictBoundary: 'projection-buffer',
			}),
		);
	}
	cases.push(
		packedProjectionFailureCase({
			bytes: mutatedBytes(projectionBytes, 7, [0x80]),
			code: 'unknown-required-flag',
			codec: 'OBPB',
			name: 'obpb-unknown-required-header-flag-80',
			strictBoundary: 'projection-buffer',
		}),
	);

	for (const [flag, name] of [
		[0x01, 'obrw-optional-header-flag-01'],
		[0x7f, 'obrw-optional-header-flags-7f'],
	]) {
		cases.push(
			packedProjectionSuccessCase({
				bytes: mutatedBytes(rowWindowBytes, 7, [flag]),
				codec: 'OBRW',
				name,
				observations: {
					detail: 'text',
					ignoredOptionalFlags: flag,
					rowCount: 2,
				},
				strictBoundary: 'row-window',
			}),
		);
	}
	cases.push(
		packedProjectionFailureCase({
			bytes: mutatedBytes(rowWindowBytes, 7, [0x80]),
			code: 'unknown-required-flag',
			codec: 'OBRW',
			name: 'obrw-unknown-required-header-flag-80',
			strictBoundary: 'row-window',
		}),
	);

	const renderView = new DataView(
		renderRowWindowBytes.buffer,
		renderRowWindowBytes.byteOffset,
		renderRowWindowBytes.byteLength,
	);
	const renderProjectionLength = renderView.getUint32(101, true);
	const renderGraphicsLength = renderView.getUint32(105, true);
	const projectionSectionMismatch = renderRowWindowBytes.slice();
	const projectionSectionView = new DataView(projectionSectionMismatch.buffer);
	projectionSectionView.setUint32(101, renderProjectionLength - 1, true);
	projectionSectionView.setUint32(105, renderGraphicsLength + 1, true);
	const graphicsSectionMismatch = renderRowWindowBytes.slice();
	new DataView(graphicsSectionMismatch.buffer).setUint32(105, renderGraphicsLength - 1, true);
	const graphicsUpdateNotSnapshot = renderRowWindowBytes.slice();
	graphicsUpdateNotSnapshot[112 + renderProjectionLength + 7] = 2;
	cases.push(
		packedProjectionFailureCase({
			bytes: projectionSectionMismatch,
			code: 'length-mismatch',
			codec: 'OBRW',
			name: 'obrw-projection-section-length-mismatch',
			strictBoundary: 'row-window',
		}),
		packedProjectionFailureCase({
			bytes: graphicsSectionMismatch,
			code: 'length-mismatch',
			codec: 'OBRW',
			name: 'obrw-graphics-section-length-mismatch',
			strictBoundary: 'row-window',
		}),
		packedProjectionFailureCase({
			bytes: graphicsUpdateNotSnapshot,
			code: 'invalid-field',
			codec: 'OBRW',
			name: 'obrw-graphics-update-not-snapshot',
			strictBoundary: 'row-window',
		}),
	);

	cases.push(
		packedProjectionSuccessCase({
			bytes: encodedTextRowWindow(
				authoredTextRowWindow({
					columns: 8,
					rows: [
						{
							index: 100n,
							mappings: identityCellMappings(5),
							promptClass: 'none',
							text: 'alpha',
							wrap: false,
							wrapContinuation: false,
						},
					],
				}),
				'conformance identity-ascii row window',
			),
			codec: 'OBRW',
			name: 'obrw-text-identity-ascii',
			observations: textObservation(1),
			strictBoundary: 'row-window',
		}),
		packedProjectionSuccessCase({
			bytes: encodedTextRowWindow(
				authoredTextRowWindow({
					columns: 4,
					rows: [
						{
							index: 100n,
							mappings: [
								{ byteOffset: 0, column: 0, utf16Offset: 0, width: 1 },
								{ byteOffset: 8192, column: 1, utf16Offset: 8192, width: 1 },
								{ byteOffset: 16384, column: 2, utf16Offset: 16384, width: 1 },
							],
							promptClass: 'none',
							text: `${'a'.repeat(8192)}${'b'.repeat(8192)}${'c'.repeat(8192)}`,
							wrap: false,
							wrapContinuation: false,
						},
					],
				}),
				'conformance aggregate row window',
			),
			codec: 'OBRW',
			name: 'obrw-text-aggregate-row-over-16k',
			observations: textObservation(1),
			strictBoundary: 'row-window',
		}),
		packedProjectionSuccessCase({
			bytes: encodedTextRowWindow(
				authoredTextRowWindow({
					columns: 4,
					rows: [
						{
							index: 100n,
							mappings: [],
							promptClass: 'none',
							text: '',
							wrap: false,
							wrapContinuation: false,
						},
					],
				}),
				'conformance empty row window',
			),
			codec: 'OBRW',
			name: 'obrw-text-empty-row',
			observations: textObservation(1),
			strictBoundary: 'row-window',
		}),
		packedProjectionSuccessCase({
			bytes: encodedTextRowWindow(
				authoredTextRowWindow({
					columns: 4,
					rows: [
						{
							index: 100n,
							mappings: identityCellMappings(1),
							promptClass: 'none',
							text: 'a',
							wrap: true,
							wrapContinuation: false,
						},
						{
							index: 101n,
							mappings: identityCellMappings(1),
							promptClass: 'none',
							text: 'b',
							wrap: true,
							wrapContinuation: true,
						},
						{
							index: 102n,
							mappings: identityCellMappings(1),
							promptClass: 'none',
							text: 'c',
							wrap: false,
							wrapContinuation: true,
						},
					],
				}),
				'conformance wrap-chain row window',
			),
			codec: 'OBRW',
			name: 'obrw-text-wrap-chain-three-rows',
			observations: textObservation(3),
			strictBoundary: 'row-window',
		}),
		packedProjectionSuccessCase({
			bytes: encodedTextRowWindow(
				authoredTextRowWindow({
					columns: 4,
					rows: [
						{
							index: 100n,
							mappings: [
								{ byteOffset: 0, column: 0, utf16Offset: 0, width: 1 },
								{ byteOffset: 3, column: 1, utf16Offset: 1, width: 1 },
							],
							promptClass: 'none',
							text: '﻿A',
							wrap: false,
							wrapContinuation: false,
						},
					],
				}),
				'conformance utf-8 leading BOM row window',
			),
			codec: 'OBRW',
			name: 'obrw-text-utf8-bom-leading',
			observations: textObservation(1),
			strictBoundary: 'row-window',
		}),
	);

	return { cases, formatVersion: rowWindowVersion, schemaVersion: 1 };
}

// A generation the bindings do not speak. Each case is a canonical record whose generation
// component alone is replaced, so the refusal cannot come from any other field.
const OTHER_GENERATION = 2;

function otherGenerationConformanceCorpus({
	envelopeBytes,
	frameBytes,
	projectionBytes,
	renderRowWindowBytes,
	rowWindowBytes,
}) {
	const base64 = (bytes) => Buffer.from(bytes).toString('base64');
	const withFormatGeneration = (bytes) => {
		const mutated = bytes.slice();
		mutated[4] = OTHER_GENERATION;
		return mutated;
	};
	const envelope = envelopeBytes.slice();
	new DataView(envelope.buffer).setUint16(16, OTHER_GENERATION, true);
	const renderView = rowWindowView(renderRowWindowBytes);
	const graphicsOffset = ROW_WINDOW_HEADER_BYTES + renderView.getUint32(101, true);
	const graphicsBytes = renderRowWindowBytes.slice(
		graphicsOffset,
		graphicsOffset + renderView.getUint32(105, true),
	);
	const refused = { code: 'unsupported-version', result: 'failure' };
	return {
		cases: [
			{
				bytesBase64: base64(envelope),
				codec: 'OBTP',
				// The envelope codec carries the version so the receiver can answer with it.
				expected: {
					protocol: { ...CURRENT_PROTOCOL_VERSION, major: OTHER_GENERATION },
					result: 'receiver-refusal',
				},
				name: 'obtp-other-generation',
			},
			{
				bytesBase64: base64(withFormatGeneration(frameBytes)),
				codec: 'OBTF',
				expected: refused,
				name: 'obtf-other-generation',
			},
			{
				bytesBase64: base64(withFormatGeneration(projectionBytes)),
				codec: 'OBPB',
				expected: refused,
				name: 'obpb-other-generation',
			},
			{
				bytesBase64: base64(withFormatGeneration(rowWindowBytes)),
				codec: 'OBRW',
				expected: refused,
				name: 'obrw-other-generation',
			},
			{
				bytesBase64: base64(withFormatGeneration(graphicsBytes)),
				codec: 'OBKG',
				expected: refused,
				name: 'obkg-other-generation',
			},
		],
		schemaVersion: 1,
	};
}

function stringifyPackedProjectionConformance(corpus) {
	const rowIndexArrays = [];
	let text = JSON.stringify(
		corpus,
		(key, value) => {
			if (key !== 'rowIndices') return value;
			const marker = `__ORCHBEE_ROW_INDICES_${rowIndexArrays.length}__`;
			rowIndexArrays.push({ marker, values: value });
			return marker;
		},
		'\t',
	);
	for (const rowIndices of rowIndexArrays) {
		text = text.replace(`"${rowIndices.marker}"`, `[${rowIndices.values.join(', ')}]`);
	}
	return `${text}\n`;
}

async function compareFile(filePath, expected) {
	let actual;
	try {
		actual = await readFile(filePath);
	} catch (error) {
		if (error && typeof error === 'object' && 'code' in error && error.code === 'ENOENT') {
			throw new Error(
				`${path.relative(packageRoot, filePath)} is missing; run pnpm fixtures:generate`,
				{ cause: error },
			);
		}
		throw error;
	}
	if (!actual.equals(Buffer.from(expected))) {
		throw new Error(`${path.relative(packageRoot, filePath)} is stale; run pnpm fixtures:generate`);
	}
}

const canonical = JSON.parse(await readFile(canonicalPath, 'utf8'));
const frameBytes = encodeOrThrow(
	encodeTerminalFrame(frameFromJson(canonical.frame)),
	'terminal frame',
);
const projectionBytes = encodeOrThrow(
	encodeTerminalProjectionBuffer({ ...frameFromJson(canonical.frame), extensions: [] }),
	'terminal projection buffer',
);
const rowWindowBytes = encodeOrThrow(
	encodeTerminalRowWindow(rowWindowFromJson(canonical.rowWindow)),
	'terminal row window',
);
const renderSnapshot = {
	...frameFromJson(canonical.frame),
	baseRevision: null,
	extensions: [],
	kind: 'snapshot',
};
const renderRowWindowBytes = encodeOrThrow(
	encodeTerminalRowWindow({
		attachmentId: renderSnapshot.attachmentId,
		columns: renderSnapshot.columns,
		detail: 'render',
		frame: renderSnapshot,
		generation: renderSnapshot.generation,
		graphics: {
			animationTick: 0n,
			images: [],
			kind: 'snapshot',
			placements: [],
			resourceRevision: 0n,
		},
		hostEpoch: renderSnapshot.hostEpoch,
		revision: renderSnapshot.revision,
		sequence: renderSnapshot.sequence,
		startRow: renderSnapshot.scrollbackRows,
		totalRows: renderSnapshot.scrollbackRows + BigInt(renderSnapshot.rows),
	}),
	'render terminal row window',
);
const envelopeBytes = encodeOrThrow(
	encodeProtocolEnvelope({
		...canonical.transportEnvelope,
		payload: frameBytes,
		protocol: CURRENT_PROTOCOL_VERSION,
	}),
	'transport envelope',
);
const inputBytes = encodeOrThrow(
	encodeInputRequest(MessageKind.writeInput, {
		attachmentId: canonical.writeInput.attachmentId,
		bytes: bytesFromBase64(canonical.writeInput.bytesBase64),
		capabilityRevision: canonical.writeInput.capabilityRevision,
	}),
	'write input',
);
const keyInputBytes = encodeOrThrow(
	encodeInputRequest(MessageKind.keyInput, canonical.keyInput),
	'key input',
);
const textInputBytes = encodeOrThrow(
	encodeInputRequest(MessageKind.textInput, canonical.textInput),
	'text input',
);
const mouseInputBytes = encodeOrThrow(
	encodeInputRequest(MessageKind.mouseInput, canonical.mouseInput),
	'mouse input',
);
const focusInputBytes = encodeOrThrow(
	encodeInputRequest(MessageKind.focusInput, canonical.focusInput),
	'focus input',
);
const pasteInputBytes = encodeOrThrow(
	encodeInputRequest(MessageKind.pasteInput, canonical.pasteInput),
	'paste input',
);
const pasteAutoInputBytes = encodeOrThrow(
	encodeInputRequest(MessageKind.pasteAutoInput, {
		attachmentId: canonical.pasteAutoInput.attachmentId,
		capabilityRevision: canonical.pasteAutoInput.capabilityRevision,
		content: bytesFromBase64(canonical.pasteAutoInput.contentBase64),
	}),
	'paste auto input',
);
const promptLineInputBytes = promptLineInputFixture(canonical.promptLineInput);
const selectionExportChunkBytes = encodeOrThrow(
	encodeSelectionExportChunk({
		attachmentId: canonical.selectionExportChunk.attachmentId,
		bytes: bytesFromBase64(canonical.selectionExportChunk.bytesBase64),
		chunkSequence: canonical.selectionExportChunk.chunkSequence,
		exportSequence: BigInt(canonical.selectionExportChunk.exportSequence),
		final: canonical.selectionExportChunk.final,
	}),
	'selection export chunk',
);
const terminalFailedBytes = new TextEncoder().encode(
	`${JSON.stringify(TerminalFailedSchema.parse(canonical.terminalFailed), null, '\t')}\n`,
);
const controlConformanceBytes = new TextEncoder().encode(
	`${JSON.stringify(createControlConformanceFixture(), null, '\t')}\n`,
);
const utf8BomConformanceBytes = new TextEncoder().encode(
	`${JSON.stringify(createUtf8BomConformanceFixture(), null, '\t')}\n`,
);
const malformedBytes = new TextEncoder().encode(
	`${JSON.stringify({ cases: malformedCorpus(frameBytes) }, null, '\t')}\n`,
);
const malformedRowWindowBytes = new TextEncoder().encode(
	`${JSON.stringify({ cases: malformedRowWindowCorpus(rowWindowBytes, renderRowWindowBytes) }, null, '\t')}\n`,
);
const packedProjectionConformanceBytes = new TextEncoder().encode(
	stringifyPackedProjectionConformance(
		packedProjectionConformanceCorpus(
			projectionBytes,
			rowWindowBytes,
			renderRowWindowBytes,
			canonical.frame,
		),
	),
);
const otherGenerationConformanceBytes = new TextEncoder().encode(
	`${JSON.stringify(
		otherGenerationConformanceCorpus({
			envelopeBytes,
			frameBytes,
			projectionBytes,
			renderRowWindowBytes,
			rowWindowBytes,
		}),
		null,
		'\t',
	)}\n`,
);

if (checkOnly) {
	await compareFile(framePath, frameBytes);
	await compareFile(projectionPath, projectionBytes);
	await compareFile(envelopePath, envelopeBytes);
	await compareFile(inputPath, inputBytes);
	await compareFile(keyInputPath, keyInputBytes);
	await compareFile(textInputPath, textInputBytes);
	await compareFile(mouseInputPath, mouseInputBytes);
	await compareFile(focusInputPath, focusInputBytes);
	await compareFile(pasteInputPath, pasteInputBytes);
	await compareFile(pasteAutoInputPath, pasteAutoInputBytes);
	await compareFile(promptLineInputPath, promptLineInputBytes);
	await compareFile(selectionExportChunkPath, selectionExportChunkBytes);
	await compareFile(terminalFailedPath, terminalFailedBytes);
	await compareFile(controlConformancePath, controlConformanceBytes);
	await compareFile(utf8BomConformancePath, utf8BomConformanceBytes);
	await compareFile(malformedPath, malformedBytes);
	await compareFile(rowWindowPath, rowWindowBytes);
	await compareFile(malformedRowWindowPath, malformedRowWindowBytes);
	await compareFile(packedProjectionConformancePath, packedProjectionConformanceBytes);
	await compareFile(otherGenerationConformancePath, otherGenerationConformanceBytes);
} else {
	await writeFile(framePath, frameBytes);
	await writeFile(projectionPath, projectionBytes);
	await writeFile(envelopePath, envelopeBytes);
	await writeFile(inputPath, inputBytes);
	await writeFile(keyInputPath, keyInputBytes);
	await writeFile(textInputPath, textInputBytes);
	await writeFile(mouseInputPath, mouseInputBytes);
	await writeFile(focusInputPath, focusInputBytes);
	await writeFile(pasteInputPath, pasteInputBytes);
	await writeFile(pasteAutoInputPath, pasteAutoInputBytes);
	await writeFile(promptLineInputPath, promptLineInputBytes);
	await writeFile(selectionExportChunkPath, selectionExportChunkBytes);
	await writeFile(terminalFailedPath, terminalFailedBytes);
	await writeFile(controlConformancePath, controlConformanceBytes);
	await writeFile(utf8BomConformancePath, utf8BomConformanceBytes);
	await writeFile(malformedPath, malformedBytes);
	await writeFile(rowWindowPath, rowWindowBytes);
	await writeFile(malformedRowWindowPath, malformedRowWindowBytes);
	await writeFile(packedProjectionConformancePath, packedProjectionConformanceBytes);
	await writeFile(otherGenerationConformancePath, otherGenerationConformanceBytes);
}
