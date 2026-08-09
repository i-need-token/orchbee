import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import {
	AttachmentCapabilitiesSchema,
	AttachTerminalSchema,
	CanonicalUint64TextSchema,
	DEFAULT_TERMINAL_CONFIGURATION,
	decodeControlJson,
	encodeControlJson,
	FailureContainmentSchema,
	FrameAppliedSchema,
	HelloAcceptedSchema,
	HelloSchema,
	InputAcceptedSchema,
	ListTerminalsSchema,
	NeedSnapshotSchema,
	ProcessObservationSchema,
	ReadRowsSchema,
	ResizeTerminalSchema,
	RowsAppliedSchema,
	RuntimeCleanupFailureSchema,
	RuntimeFailureSchema,
	SelectionChangedSchema,
	SelectionExportSchema,
	SetTerminalDefaultsSchema,
	SetProjectionDemandSchema,
	TerminalConfigurationSchema,
	TerminalFailedSchema,
	UpdateTerminalConfigurationSchema,
	TerminalLaunchSchema,
	TerminalListSchema,
	TerminalSummarySchema,
	TypedFailureSchema,
} from '../control';
import { ProtocolErrorCode } from '../protocol-result';

const profile = {
	colorScheme: 'dark',
	defaultBackgroundRgb: 0,
	defaultCursorRgb: null,
	defaultForegroundRgb: 0xffffff,
	deviceAttributes: {
		primaryConformanceLevel: 62,
		primaryFeatures: [22],
		secondaryDeviceType: 1,
		secondaryFirmwareVersion: 0,
		secondaryRomCartridge: 0,
		tertiaryUnitId: 0,
	},
	enquiryResponse: '',
	palette: Array.from({ length: 256 }, (_, index) => index),
	versionResponse: 'orchbee',
} as const;

const launch = {
	arguments: ['--version'],
	cellPixelHeight: 16,
	cellPixelWidth: 8,
	columns: 80,
	configuration: null,
	currentDirectory: null,
	environment: [
		{ name: 'EMPTY', value: '' },
		{ name: 'REMOVED', value: null },
	],
	executable: '/bin/example',
	rows: 24,
	terminalProfile: profile,
};

const nestedObjectSchema = z.object({
	a: z.object({ x: z.number() }),
	b: z.object({ x: z.number() }),
});
const currentProtocolVersion = { major: 0, minor: 0, patch: 1 } as const;

describe('control schemas', () => {
	it('requires every attachment to carry meaningful authority', () => {
		const attachment = {
			attachmentId: '11'.repeat(16),
			capabilities: { input: false, read: true, resize: false },
			requestResizeLease: false,
			terminalId: '22'.repeat(16),
		};
		expect(AttachTerminalSchema.safeParse(attachment).success).toBe(true);
		expect(
			AttachTerminalSchema.safeParse({
				...attachment,
				capabilities: { input: false, read: false, resize: false },
			}).success,
		).toBe(false);
		expect(
			AttachTerminalSchema.safeParse({ ...attachment, requestResizeLease: true }).success,
		).toBe(false);
		expect(
			AttachmentCapabilitiesSchema.safeParse({
				attachmentId: attachment.attachmentId,
				capabilities: attachment.capabilities,
				resizeLease: false,
				terminalId: attachment.terminalId,
			}).success,
		).toBe(true);
		expect(
			AttachmentCapabilitiesSchema.safeParse({
				attachmentId: attachment.attachmentId,
				capabilities: { input: true, read: false, resize: false },
				resizeLease: true,
				terminalId: attachment.terminalId,
			}).success,
		).toBe(false);
	});

	it('preserves absent and explicitly empty environment values', () => {
		expect(TerminalLaunchSchema.parse(launch)).toEqual(launch);
		expect(
			TerminalLaunchSchema.safeParse({
				...launch,
				environment: [...launch.environment, { name: 'EMPTY', value: 'again' }],
			}).success,
		).toBe(false);
		expect(
			TerminalLaunchSchema.safeParse({
				...launch,
				environment: [{ name: 'INVALID\0NAME', value: 'value' }],
			}).success,
		).toBe(false);
		expect(
			TerminalLaunchSchema.safeParse({ ...launch, executable: '/bin/bad\0name' }).success,
		).toBe(false);
		expect(TerminalLaunchSchema.safeParse({ ...launch, executable: '/bin/\ud800' }).success).toBe(
			false,
		);
		expect(
			TerminalLaunchSchema.safeParse({
				...launch,
				environment: [{ name: 'BAD\ud800', value: null }],
			}).success,
		).toBe(false);
		expect(
			TerminalLaunchSchema.safeParse({
				...launch,
				terminalProfile: { ...profile, enquiryResponse: '\ud800' },
			}).success,
		).toBe(false);
		expect(
			SelectionChangedSchema.safeParse({
				attachmentId: '22'.repeat(16),
				end: { column: 1, row: '1' },
				preview: '\ud800',
				rectangle: false,
				start: { column: 0, row: '1' },
				state: 'active',
			}).success,
		).toBe(false);
		expect(
			TypedFailureSchema.safeParse({
				code: 'invalid-state',
				message: '\ud800',
				retryable: false,
			}).success,
		).toBe(false);
	});

	it('bounds direct schema text by encoded bytes', () => {
		expect(
			TerminalLaunchSchema.safeParse({
				...launch,
				executable: '🐝'.repeat(20_000),
			}).success,
		).toBe(false);
		expect(
			TerminalLaunchSchema.safeParse({
				...launch,
				environment: [{ name: '🐝'.repeat(9_000), value: null }],
			}).success,
		).toBe(false);
		expect(
			TypedFailureSchema.safeParse({
				code: '🐝'.repeat(33),
				message: 'failure',
				retryable: false,
			}).success,
		).toBe(false);
		expect(
			TypedFailureSchema.safeParse({
				code: 'failure',
				message: '🐝'.repeat(257),
				retryable: false,
			}).success,
		).toBe(false);
	});

	it('validates canonical u64 text and strict UTF-8 JSON', () => {
		expect(CanonicalUint64TextSchema.safeParse('18446744073709551615').success).toBe(true);
		expect(CanonicalUint64TextSchema.safeParse('01').success).toBe(false);
		for (const invalid of ['nope', '1.2', '-1', '18446744073709551616']) {
			expect(() => CanonicalUint64TextSchema.safeParse(invalid)).not.toThrow();
			expect(CanonicalUint64TextSchema.safeParse(invalid).success).toBe(false);
		}
		expect(() =>
			decodeControlJson(
				new TextEncoder().encode(
					JSON.stringify({
						attachmentId: '22'.repeat(16),
						generation: 'not-an-integer',
						revision: '2',
						sequence: '3',
					}),
				),
				FrameAppliedSchema,
			),
		).not.toThrow();
		expect(
			decodeControlJson(
				new TextEncoder().encode(
					JSON.stringify({
						attachmentId: '22'.repeat(16),
						generation: 'not-an-integer',
						revision: '2',
						sequence: '3',
					}),
				),
				FrameAppliedSchema,
			),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));
		expect(
			decodeControlJson(new TextEncoder().encode(JSON.stringify(launch)), TerminalLaunchSchema),
		).toEqual({ ok: true, value: launch });
		expect(decodeControlJson(new Uint8Array([0xff]), TerminalLaunchSchema)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidUtf8, ok: false }),
		);
	});

	it('rejects zero for generation and attachment-local flight sequences', () => {
		const attachmentId = '22'.repeat(16);
		const frameApplied = { attachmentId, generation: '1', revision: '0', sequence: '1' };
		expect(FrameAppliedSchema.safeParse(frameApplied).success).toBe(true);
		expect(FrameAppliedSchema.safeParse({ ...frameApplied, generation: '0' }).success).toBe(false);
		expect(FrameAppliedSchema.safeParse({ ...frameApplied, sequence: '0' }).success).toBe(false);
		expect(
			NeedSnapshotSchema.safeParse({
				attachmentId,
				reason: 'sequence-gap',
				sequence: '0',
			}).success,
		).toBe(false);

		const readRows = {
			attachmentId,
			detail: 'text',
			generation: '1',
			maximumBytes: 1024,
			origin: { kind: 'tail', rowsBefore: 0 },
			purpose: 'viewport',
			requestSequence: '1',
			revision: '0',
			rowCount: 1,
		};
		expect(ReadRowsSchema.safeParse(readRows).success).toBe(true);
		expect(ReadRowsSchema.safeParse({ ...readRows, requestSequence: '0' }).success).toBe(false);
		expect(
			RowsAppliedSchema.safeParse({
				attachmentId,
				generation: '1',
				requestSequence: '0',
				revision: '0',
			}).success,
		).toBe(false);
		expect(
			SelectionExportSchema.safeParse({
				attachmentId,
				exportSequence: '0',
				format: 'plain',
			}).success,
		).toBe(false);
	});

	it('validates and bounds control values before encoding', () => {
		const encoded = encodeControlJson(launch, TerminalLaunchSchema);
		expect(encoded.ok).toBe(true);
		if (!encoded.ok) throw new Error(encoded.detail);
		expect(decodeControlJson(encoded.value, TerminalLaunchSchema)).toEqual({
			ok: true,
			value: launch,
		});
		expect(
			encodeControlJson({ ...launch, executable: '/bin/\ud800' }, TerminalLaunchSchema),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }));
		expect(encodeControlJson({ text: 'x'.repeat(65_536) }, z.object({ text: z.string() }))).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.limitExceeded, ok: false }),
		);
		expect(encodeControlJson(1n, z.unknown())).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});

	it('bounds optional feature names by UTF-8 bytes', () => {
		const hello = {
			clientId: '33333333333333333333333333333333',
			frameBytes: 1024,
			inputBytes: 1024,
			protocol: currentProtocolVersion,
			rowWindowBytes: 1024,
			supportedOptionalFeatures: ['😀'.repeat(16)],
		};
		expect(HelloSchema.safeParse({ ...hello, supportedOptionalFeatures: ['a'] }).success).toBe(
			true,
		);
		expect(HelloSchema.safeParse({ ...hello, supportedOptionalFeatures: [''] }).success).toBe(
			false,
		);
		expect(HelloSchema.safeParse(hello).success).toBe(true);
		expect(
			HelloSchema.safeParse({
				...hello,
				supportedOptionalFeatures: ['😀'.repeat(17)],
			}).success,
		).toBe(false);
		expect(
			HelloSchema.safeParse({
				...hello,
				supportedOptionalFeatures: ['e\u0301'.repeat(22)],
			}).success,
		).toBe(false);
		expect(HelloSchema.safeParse({ ...hello, supportedOptionalFeatures: ['\ud800'] }).success).toBe(
			false,
		);
		expect(
			HelloSchema.safeParse({
				...hello,
				supportedOptionalFeatures: Array.from({ length: 64 }, (_, index) => `f${index}`),
			}).success,
		).toBe(true);
		expect(
			HelloSchema.safeParse({
				...hello,
				supportedOptionalFeatures: Array.from({ length: 65 }, (_, index) => `f${index}`),
			}).success,
		).toBe(false);
		expect(
			HelloAcceptedSchema.safeParse({
				frameBytes: 1024,
				hostEpoch: '44444444444444444444444444444444',
				hostVersion: '0.1.0',
				inputBytes: 1024,
				protocol: currentProtocolVersion,
				rowWindowBytes: 1024,
				supportedOptionalFeatures: ['a'.repeat(64)],
			}).success,
		).toBe(true);
		expect(
			HelloAcceptedSchema.safeParse({
				frameBytes: 1024,
				hostEpoch: '44444444444444444444444444444444',
				hostVersion: '0.1.0',
				inputBytes: 1024,
				protocol: currentProtocolVersion,
				rowWindowBytes: 1024,
				supportedOptionalFeatures: [''],
			}).success,
		).toBe(false);
		expect(
			HelloAcceptedSchema.safeParse({
				frameBytes: 1024,
				hostEpoch: '44444444444444444444444444444444',
				hostVersion: '0.1.0',
				inputBytes: 1024,
				protocol: currentProtocolVersion,
				rowWindowBytes: 1024,
				supportedOptionalFeatures: ['\ud800'],
			}).success,
		).toBe(false);
		expect(
			HelloAcceptedSchema.safeParse({
				frameBytes: 1024,
				hostEpoch: '44444444444444444444444444444444',
				hostVersion: '0.1.0',
				inputBytes: 1024,
				protocol: currentProtocolVersion,
				rowWindowBytes: 1024,
				supportedOptionalFeatures: Array.from({ length: 65 }, (_, index) => `f${index}`),
			}).success,
		).toBe(false);
	});

	it('rejects duplicate JSON members, including escaped-equivalent keys', () => {
		const validHello = JSON.stringify({
			clientId: '33333333333333333333333333333333',
			frameBytes: 1024,
			inputBytes: 1024,
			protocol: currentProtocolVersion,
			rowWindowBytes: 1024,
			supportedOptionalFeatures: [],
		});
		const duplicateRoot = validHello.replace(
			'"frameBytes":1024',
			'"frameBytes":1024,"frameBytes":1024',
		);
		const duplicateNested = validHello.replace(
			'"major":0,"minor":0,"patch":1',
			'"major":0,"minor":0,"minor":0,"patch":1',
		);
		expect(decodeControlJson(new TextEncoder().encode(duplicateRoot), HelloSchema)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidJson, ok: false }),
		);
		expect(decodeControlJson(new TextEncoder().encode(duplicateNested), HelloSchema)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidJson, ok: false }),
		);
		expect(
			decodeControlJson(
				new TextEncoder().encode('{"protocol":{"major":0,"minor":0,"mi\\u006eor":0}}'),
				HelloSchema,
			),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidJson, ok: false }));
		expect(
			decodeControlJson(new TextEncoder().encode('{"items":[{"x":1,"x":2}]}'), nestedObjectSchema),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidJson, ok: false }));
		expect(
			decodeControlJson(
				new TextEncoder().encode(
					JSON.stringify({ ...JSON.parse(validHello), supportedOptionalFeatures: ['\ud800'] }),
				),
				HelloSchema,
			),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidJson, ok: false }));
		expect(
			decodeControlJson(new TextEncoder().encode('{"a":{"x":1},"b":{"x":2}}'), nestedObjectSchema)
				.ok,
		).toBe(true);
	});

	it('requires resolved emulation and pixel geometry on the current launch', () => {
		expect(TerminalLaunchSchema.safeParse(launch).success).toBe(true);
		expect(
			TerminalLaunchSchema.safeParse({
				...launch,
				columns: 4096,
				rows: 4096,
			}).success,
		).toBe(false);
	});

	it('validates projection demand, recovery acknowledgements, and pixel-only resize', () => {
		expect(
			SetProjectionDemandSchema.safeParse({
				attachmentId: '22222222222222222222222222222222',
				demand: 'active',
			}).success,
		).toBe(true);
		expect(
			FrameAppliedSchema.safeParse({
				attachmentId: '22222222222222222222222222222222',
				generation: '1',
				revision: '2',
				sequence: '3',
			}).success,
		).toBe(true);
		expect(
			NeedSnapshotSchema.safeParse({
				attachmentId: '22222222222222222222222222222222',
				reason: 'base-mismatch',
				sequence: '3',
			}).success,
		).toBe(true);
		expect(
			ResizeTerminalSchema.safeParse({
				attachmentId: '22222222222222222222222222222222',
				cellPixelHeight: 18,
				cellPixelWidth: 9,
				columns: 80,
				rows: 24,
			}).success,
		).toBe(true);
		expect(
			InputAcceptedSchema.safeParse({
				acceptedBytes: 0,
				attachmentId: '22222222222222222222222222222222',
			}).success,
		).toBe(true);
	});

	it('bounds current inventory pages without bounding total inventory', () => {
		const request = {
			cursor: {
				inventoryGeneration: '7',
				offset: '128',
			},
			maximumTerminals: 128,
		};
		expect(ListTerminalsSchema.parse(request)).toEqual(request);
		expect(ListTerminalsSchema.safeParse({ ...request, maximumTerminals: 129 }).success).toBe(
			false,
		);

		const response = {
			inventoryGeneration: '7',
			nextCursor: {
				inventoryGeneration: '7',
				offset: '256',
			},
			terminals: [],
		};
		expect(TerminalListSchema.parse(response)).toEqual(response);
		expect(
			TerminalListSchema.safeParse({
				...response,
				nextCursor: { ...response.nextCursor, inventoryGeneration: '8' },
			}).success,
		).toBe(false);
	});

	it('separates user scrollback configuration from protocol hard limits', () => {
		expect(DEFAULT_TERMINAL_CONFIGURATION).toEqual({ scrollbackBytes: 10_000_000 });
		expect(
			SetTerminalDefaultsSchema.safeParse({ configuration: { scrollbackBytes: 25_000_000 } })
				.success,
		).toBe(true);
		expect(
			UpdateTerminalConfigurationSchema.safeParse({
				configuration: { scrollbackBytes: 0 },
				terminalId: '22222222222222222222222222222222',
			}).success,
		).toBe(true);
		expect(TerminalConfigurationSchema.safeParse({ scrollbackBytes: -1 }).success).toBe(false);
		expect(TerminalConfigurationSchema.safeParse({ scrollbackBytes: 1_000_000_001 }).success).toBe(
			false,
		);
	});

	it('makes normal, containing, and failed terminal states mutually exclusive', () => {
		const common = {
			columns: 80,
			configuration: DEFAULT_TERMINAL_CONFIGURATION,
			configurationRevision: '1',
			identity: {
				creationId: '11'.repeat(16),
				hostEpoch: '22'.repeat(16),
				terminalId: '33'.repeat(16),
			},
			revision: '7',
			rows: 24,
		};
		const failure = {
			kind: 'pty-read',
			stage: 'read-output',
		} as const;
		const process = {
			kind: 'observed',
			result: { code: 1, kind: 'exit-code' },
		} as const;
		const failed = {
			...common,
			exit: null,
			failure,
			failureContainment: null,
			phase: 'failed',
			process,
			terminateRequested: true,
		};

		expect(TerminalSummarySchema.parse(failed)).toEqual(failed);
		expect(RuntimeFailureSchema.safeParse(failure).success).toBe(true);
		expect(
			TerminalFailedSchema.safeParse({
				failure,
				finalRevision: common.revision,
				identity: common.identity,
				process,
			}).success,
		).toBe(true);
		expect(
			TerminalFailedSchema.safeParse({
				failure: { ...failure, stage: 'apply-resize' },
				finalRevision: common.revision,
				identity: common.identity,
				process,
			}).success,
		).toBe(false);
		expect(
			ProcessObservationSchema.safeParse({
				failure,
				kind: 'unavailable',
			}).success,
		).toBe(false);
		expect(
			FailureContainmentSchema.safeParse({
				cleanupFailure: failure,
				kind: 'blocked',
				process: null,
			}).success,
		).toBe(false);
		const cleanupFailures = [
			{ kind: 'process-control', stage: 'contain-process' },
			{ kind: 'pty-read', stage: 'release-output' },
			{ kind: 'process-observation', stage: 'release-process-identity' },
		] as const;
		for (const cleanupFailure of cleanupFailures) {
			expect(RuntimeCleanupFailureSchema.safeParse(cleanupFailure).success).toBe(true);
			expect(RuntimeFailureSchema.safeParse(cleanupFailure).success).toBe(false);
			expect(
				FailureContainmentSchema.safeParse({
					cleanupFailure,
					kind: 'blocked',
					process: null,
				}).success,
			).toBe(true);
			expect(
				TerminalFailedSchema.safeParse({
					failure: cleanupFailure,
					finalRevision: common.revision,
					identity: common.identity,
					process,
				}).success,
			).toBe(false);
		}
		expect(
			TerminalSummarySchema.safeParse({
				...failed,
				exit: { code: 0, kind: 'exit-code' },
			}).success,
		).toBe(false);
		expect(
			TerminalSummarySchema.safeParse({
				...common,
				exit: null,
				failure,
				failureContainment: null,
				phase: 'containing-failure',
				process: null,
				terminateRequested: true,
			}).success,
		).toBe(false);
	});
});
