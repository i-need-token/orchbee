import { readFile, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import path from 'node:path';

import {
	AttachmentAcknowledgedSchema,
	AttachmentCapabilitiesSchema,
	CURRENT_PROTOCOL_VERSION,
	decodeKeyInput,
	decodePasteInput,
	decodeTextInput,
	EmptyControlSchema,
	encodeControlJson,
	encodeControlRequest,
	encodeInputRequest,
	encodeProtocolEnvelope,
	encodeTerminalProjectionBuffer,
	encodeTerminalRowWindow,
	encodeTerminalFrame,
	HelloAcceptedSchema,
	HistoryChangedSchema,
	HistoryDemandAcceptedSchema,
	InputAcceptedSchema,
	KEY_INPUT_HEADER_BYTES,
	ListTerminalsSchema,
	MESSAGE_CONTRACTS,
	MessageKind,
	PROTOCOL_LIMITS,
	ProjectionBlockedSchema,
	ProjectionDemandAcceptedSchema,
	ProtocolRejectedSchema,
	ResizeAcceptedSchema,
	ResizeLeaseChangedSchema,
	SelectionChangedSchema,
	SelectionExportFinishedSchema,
	SelectionExportStartedSchema,
	SetTerminalDefaultsSchema,
	TerminalAcknowledgedSchema,
	TerminalBellSchema,
	TerminalConfigurationAcceptedSchema,
	TerminalControlAcceptedSchema,
	TerminalDefaultsAcceptedSchema,
	TerminalExitedSchema,
	TerminalFailedSchema,
	TERMINAL_FRAME_FORMAT,
	TerminalIdentitySchema,
	TerminalListSchema,
	TERMINAL_PROJECTION_BUFFER_FORMAT,
	TERMINAL_ROW_WINDOW_FORMAT,
	TerminalStartedSchema,
	TerminalTargetSchema,
	TypedFailureSchema,
} from '../dist/index.js';

const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const fixturesDirectory = path.join(packageRoot, 'fixtures');

function versionText(version) {
	return `${version.major}.${version.minor}.${version.patch}`;
}

const protocolVersion = versionText(CURRENT_PROTOCOL_VERSION);
const frameVersion = versionText(TERMINAL_FRAME_FORMAT);
const projectionVersion = versionText(TERMINAL_PROJECTION_BUFFER_FORMAT);
const rowWindowVersion = versionText(TERMINAL_ROW_WINDOW_FORMAT);
const canonicalPath = path.join(fixturesDirectory, 'canonical.json');
const framePath = path.join(fixturesDirectory, `terminal-frame-v${frameVersion}.bin`);
const projectionPath = path.join(
	fixturesDirectory,
	`terminal-projection-buffer-v${projectionVersion}.bin`,
);
const envelopePath = path.join(fixturesDirectory, `transport-envelope-v${protocolVersion}.bin`);
const inputPath = path.join(fixturesDirectory, `write-input-v${protocolVersion}.bin`);
const keyInputPath = path.join(fixturesDirectory, `key-input-v${protocolVersion}.bin`);
const textInputPath = path.join(fixturesDirectory, `text-input-v${protocolVersion}.bin`);
const mouseInputPath = path.join(fixturesDirectory, `mouse-input-v${protocolVersion}.bin`);
const focusInputPath = path.join(fixturesDirectory, `focus-input-v${protocolVersion}.bin`);
const pasteInputPath = path.join(fixturesDirectory, `paste-input-v${protocolVersion}.bin`);
const terminalFailedPath = path.join(fixturesDirectory, `terminal-failed-v${protocolVersion}.json`);
const controlConformancePath = path.join(
	fixturesDirectory,
	`control-conformance-v${protocolVersion}.json`,
);
const utf8BomConformancePath = path.join(
	fixturesDirectory,
	`utf8-bom-conformance-v${protocolVersion}.json`,
);
const malformedPath = path.join(fixturesDirectory, `malformed-frame-v${frameVersion}.json`);
const rowWindowPath = path.join(fixturesDirectory, `terminal-row-window-v${rowWindowVersion}.bin`);
const malformedRowWindowPath = path.join(
	fixturesDirectory,
	`malformed-row-window-v${rowWindowVersion}.json`,
);
const checkOnly = process.argv.includes('--check');

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
			composing: false,
			consumedModifiers: modifiers,
			key: 'F1',
			modifiers,
			text: '',
			unshiftedCodepoint: 0,
		}),
		'UTF-8 BOM key input',
	);
	const bom = new Uint8Array([0xef, 0xbb, 0xbf]);
	const bomKey = new Uint8Array(key.byteLength + bom.byteLength);
	const keyBody = key.subarray(KEY_INPUT_HEADER_BYTES);
	bomKey.set(key.subarray(0, KEY_INPUT_HEADER_BYTES));
	new DataView(bomKey.buffer).setUint16(22, bom.byteLength + keyBody.byteLength, true);
	bomKey.set(bom, KEY_INPUT_HEADER_BYTES);
	bomKey.set(keyBody, KEY_INPUT_HEADER_BYTES + bom.byteLength);
	const textValue = { attachmentId, text: '\ufeff' };
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

function protocolLimitName(name) {
	const semanticName = name.startsWith('max') ? name.slice(3) : name;
	return semanticName.replaceAll(/([a-z\d])([A-Z])/g, '$1_$2').toLowerCase();
}

function protocolLimitsFixture() {
	return Object.fromEntries(
		Object.entries(PROTOCOL_LIMITS).map(([name, value]) => [protocolLimitName(name), value]),
	);
}

function createControlConformanceFixture() {
	const attachmentId = '22'.repeat(16);
	const creationId = '33'.repeat(16);
	const terminalId = '44'.repeat(16);
	const hostEpoch = '11'.repeat(16);
	const clientId = '55'.repeat(16);
	const protocol = CURRENT_PROTOCOL_VERSION;
	const terminalProfile = {
		colorScheme: 'dark',
		defaultBackgroundRgb: 0x10_10_10,
		defaultCursorRgb: null,
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
	const attachment = { attachmentId, generation: '3', revision: '42' };
	const point = { column: 4, row: '100' };
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
	const registeredMessageKinds = Object.values(MessageKind).sort((left, right) => left - right);
	if (new Set(registeredMessageKinds).size !== registeredMessageKinds.length) {
		throw new Error('The message-kind registry contains duplicate numeric values');
	}
	const messageContracts = registeredMessageKinds.map((kind) => {
		const contract = MESSAGE_CONTRACTS[kind];
		return {
			event_encoding: contract.eventEncoding,
			kind,
			name: contract.name,
			request:
				contract.request === null
					? null
					: {
							payload_encoding: contract.request.payloadEncoding,
							success_kind: contract.request.successKind,
						},
			response_encoding: contract.responseEncoding,
		};
	});

	const commands = {
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
		clear_terminal: controlRequestText(
			MessageKind.clearTerminal,
			{ attachmentId },
			'ClearTerminal',
		),
		create_terminal: controlRequestText(
			MessageKind.createTerminal,
			{ creationId, launch },
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
				rowWindowBytes: 2 * 1024 * 1024,
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
				{ detail: 'text', origin: { kind: 'absolute', row: '10' }, purpose: 'buffer' },
				{ detail: 'render', origin: { kind: 'oldest', offset: 2 }, purpose: 'search' },
				{ detail: 'text', origin: { delta: -1, kind: 'committed' }, purpose: 'viewport' },
				{ detail: 'render', origin: { kind: 'tail', rowsBefore: 3 }, purpose: 'buffer' },
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
			['html', 'plain', 'vt'].map((format, index) => ({
				attachmentId,
				exportSequence: String(index + 1),
				format,
			})),
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
					...attachment,
					clickCount: 2,
					point,
					position: { x: 36, y: 20 },
					selectOutput: false,
				},
				{
					action: 'update',
					...attachment,
					geometry: { cellWidth: 8, columns: 120, paddingLeft: 0, screenHeight: 640 },
					point,
					position: { x: 44, y: -1 },
					rectangle: true,
				},
				{ action: 'end', ...attachment, point },
				{ action: 'cancel', ...attachment },
			],
			'SelectionGesture',
		),
		set_terminal_defaults: controlRequestText(
			MessageKind.setTerminalDefaults,
			{ configuration: { scrollbackBytes: 25_000_000 } },
			'SetTerminalDefaults',
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
				capabilities: { input: true, read: true, resize: false },
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
				rowWindowBytes: 2 * 1024 * 1024,
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
			{ acceptedBytes: 12, attachmentId },
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
			{ client: { major: 0, minor: 1, patch: 0 }, host: protocol, reason: 'version-mismatch' },
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
			['html', 'plain', 'vt'].map((format, index) => ({
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
		terminal_bell: controlText(TerminalBellSchema, { attachmentId, sequence: '6' }, 'TerminalBell'),
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
		terminal_list: controlText(
			TerminalListSchema,
			{ inventoryGeneration: '9', nextCursor: null, terminals: terminalSummaries },
			'TerminalList',
		),
		terminal_started: controlText(TerminalStartedSchema, identity, 'TerminalStarted'),
		typed_failure: controlTexts(
			TypedFailureSchema,
			[
				{ code: 'invalid-state', message: 'The command is not legal now.', retryable: false },
				{ code: 'backpressure', message: 'Try again later.', retryable: true },
			],
			'TypedFailure',
		),
	};

	return {
		commands,
		message_contracts: messageContracts,
		outputs,
		protocol_limits: protocolLimitsFixture(),
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

function malformedRowWindowCorpus(rowWindowBytes) {
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

	return [
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
	];
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
	`${JSON.stringify({ cases: malformedRowWindowCorpus(rowWindowBytes) }, null, '\t')}\n`,
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
	await compareFile(terminalFailedPath, terminalFailedBytes);
	await compareFile(controlConformancePath, controlConformanceBytes);
	await compareFile(utf8BomConformancePath, utf8BomConformanceBytes);
	await compareFile(malformedPath, malformedBytes);
	await compareFile(rowWindowPath, rowWindowBytes);
	await compareFile(malformedRowWindowPath, malformedRowWindowBytes);
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
	await writeFile(terminalFailedPath, terminalFailedBytes);
	await writeFile(controlConformancePath, controlConformanceBytes);
	await writeFile(utf8BomConformancePath, utf8BomConformanceBytes);
	await writeFile(malformedPath, malformedBytes);
	await writeFile(rowWindowPath, rowWindowBytes);
	await writeFile(malformedRowWindowPath, malformedRowWindowBytes);
}
