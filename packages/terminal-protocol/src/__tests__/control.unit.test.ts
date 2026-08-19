import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import {
	ApplyTerminalThemeSchema,
	AttachmentCapabilitiesSchema,
	AttachTerminalSchema,
	canonicalEnvironmentName,
	CanonicalUint64TextSchema,
	ClearTerminalSchema,
	DEFAULT_TERMINAL_CONFIGURATION,
	decodeControlJson,
	EmptyControlSchema,
	encodeControlJson,
	FailureContainmentSchema,
	FrameAppliedSchema,
	HelloAcceptedSchema,
	HelloSchema,
	InputAcceptedSchema,
	ListTerminalsSchema,
	NeedSnapshotSchema,
	prepareLaunchEnvironment,
	ProcessObservationSchema,
	ReadRowsSchema,
	ResizeLeaseChangedSchema,
	ResizeTerminalSchema,
	ResetTerminalSchema,
	RowsAppliedSchema,
	RuntimeCleanupFailureSchema,
	RuntimeFailureSchema,
	SelectionChangedSchema,
	SelectionCommandSchema,
	SelectionExportSchema,
	SelectionGestureSchema,
	SetTerminalDefaultsSchema,
	StartTerminalSchema,
	SetProjectionDemandSchema,
	TerminalConfigurationSchema,
	TerminalControlAcceptedSchema,
	TerminalBellSchema,
	TerminalClipboardWriteRequestSchema,
	TerminalFailedSchema,
	UpdateTerminalConfigurationSchema,
	TerminalLaunchSchema,
	TerminalListSchema,
	TerminalStartAcceptedSchema,
	TerminalProfileSchema,
	TerminalSummarySchema,
	TerminalThemeResultSchema,
	TerminalThemeSchema,
	TerminalThemeStateSchema,
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

const explicitProfile = {
	...profile,
	defaultCursorBlinking: false,
	defaultCursorShape: 'block',
} as const;

const terminalTheme = {
	colorScheme: profile.colorScheme,
	defaultBackgroundRgb: profile.defaultBackgroundRgb,
	defaultCursorRgb: profile.defaultCursorRgb,
	defaultForegroundRgb: profile.defaultForegroundRgb,
	palette: profile.palette,
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
	terminalProfile: explicitProfile,
};

const nestedObjectSchema = z.object({
	a: z.object({ x: z.number() }),
	b: z.object({ x: z.number() }),
});
const currentProtocolVersion = { major: 0, minor: 0, patch: 7 } as const;

describe('control schemas', () => {
	it('normalizes legacy and explicit cursor defaults in terminal profiles', () => {
		expect(TerminalProfileSchema.parse(profile)).toEqual({
			...profile,
			defaultCursorBlinking: false,
			defaultCursorShape: 'block',
		});
		expect(
			TerminalProfileSchema.parse({
				...explicitProfile,
				defaultCursorBlinking: true,
				defaultCursorShape: 'bar',
			}),
		).toEqual({
			...explicitProfile,
			defaultCursorBlinking: true,
			defaultCursorShape: 'bar',
		});
	});

	it('models one atomic terminal theme compare-and-set and complete recovery state', () => {
		const terminalId = '22'.repeat(16);
		const operationId = '33'.repeat(16);
		const otherOperationId = '44'.repeat(16);
		const baseline = { operationId: null, revision: '0', terminal: terminalTheme } as const;
		const current = { operationId, revision: '1', terminal: terminalTheme } as const;

		expect(TerminalThemeSchema.parse(terminalTheme)).toEqual(terminalTheme);
		expect(TerminalThemeStateSchema.parse(baseline)).toEqual(baseline);
		expect(
			ApplyTerminalThemeSchema.parse({
				expectedThemeRevision: '0',
				operationId,
				terminal: terminalTheme,
				terminalId,
			}),
		).toEqual({
			expectedThemeRevision: '0',
			operationId,
			terminal: terminalTheme,
			terminalId,
		});
		expect(
			TerminalThemeResultSchema.parse({
				current,
				operationId,
				terminalId,
				transition: 'applied',
			}),
		).toEqual({ current, operationId, terminalId, transition: 'applied' });
		expect(
			TerminalThemeResultSchema.parse({
				current,
				operationId: otherOperationId,
				terminalId,
				transition: 'stale',
			}),
		).toEqual({ current, operationId: otherOperationId, terminalId, transition: 'stale' });

		expect(TerminalThemeStateSchema.safeParse({ ...baseline, operationId }).success).toBe(false);
		expect(TerminalThemeStateSchema.safeParse({ ...current, operationId: null }).success).toBe(
			false,
		);
		expect(
			TerminalThemeResultSchema.safeParse({
				current,
				operationId: otherOperationId,
				terminalId,
				transition: 'already-applied',
			}).success,
		).toBe(false);
		expect(
			TerminalThemeSchema.safeParse({ ...terminalTheme, palette: terminalTheme.palette.slice(1) })
				.success,
		).toBe(false);
	});

	it('scopes clear and reset to one attachment and acknowledges the exact operation', () => {
		const attachmentId = '11'.repeat(16);
		const terminalId = '22'.repeat(16);
		expect(ClearTerminalSchema.parse({ attachmentId })).toEqual({ attachmentId });
		expect(ResetTerminalSchema.parse({ attachmentId })).toEqual({ attachmentId });
		expect(
			TerminalControlAcceptedSchema.parse({
				attachmentId,
				generation: '3',
				operation: 'reset',
				revision: '4',
				terminalId,
			}),
		).toEqual({ attachmentId, generation: '3', operation: 'reset', revision: '4', terminalId });
		expect(ClearTerminalSchema.safeParse({ attachmentId, operation: 'clear' }).success).toBe(false);
		expect(
			TerminalControlAcceptedSchema.safeParse({
				attachmentId,
				generation: '0',
				operation: 'clear',
				revision: '4',
				terminalId,
			}).success,
		).toBe(false);
	});
	it('scopes every bell to one attachment and one positive terminal sequence', () => {
		const bell = { attachmentId: '11'.repeat(16), sequence: '7' };

		expect(TerminalBellSchema.parse(bell)).toEqual(bell);
		expect(TerminalBellSchema.safeParse({ ...bell, sequence: '0' }).success).toBe(false);
		expect(TerminalBellSchema.safeParse({ ...bell, sequence: '01' }).success).toBe(false);
		expect(TerminalBellSchema.safeParse({ sequence: '7' }).success).toBe(false);
	});

	it('bounds attachment-scoped terminal clipboard writes to valid plain text', () => {
		const request = { attachmentId: '11'.repeat(16), sequence: '8', text: 'copy me' };
		expect(TerminalClipboardWriteRequestSchema.parse(request)).toEqual(request);
		expect(
			TerminalClipboardWriteRequestSchema.safeParse({ ...request, text: '\0hidden' }).success,
		).toBe(false);
		expect(
			TerminalClipboardWriteRequestSchema.safeParse({ ...request, text: 'x'.repeat(16_385) })
				.success,
		).toBe(false);
		expect(
			TerminalClipboardWriteRequestSchema.safeParse({ ...request, sequence: '0' }).success,
		).toBe(false);
	});

	it('represents an explicit terminal-cell selection range without pointer gesture semantics', () => {
		const command = {
			attachmentId: '11'.repeat(16),
			command: 'set-range',
			end: { column: 7, row: '12' },
			generation: '3',
			rectangle: true,
			revision: '9',
			start: { column: 2, row: '10' },
		};

		expect(SelectionCommandSchema.safeParse(command).success).toBe(true);
		expect(SelectionCommandSchema.safeParse({ ...command, point: command.start }).success).toBe(
			false,
		);
	});

	it('requires action-specific selection gesture evidence', () => {
		const identity = {
			attachmentId: '11'.repeat(16),
			generation: '3',
			revision: '9',
		};
		const point = { column: 2, row: '10' };
		const start = {
			...identity,
			action: 'start',
			clickCount: 3,
			point,
			position: { x: 21, y: 4 },
			selectOutput: true,
		};
		const update = {
			...identity,
			action: 'update',
			geometry: { cellWidth: 9, columns: 80, paddingLeft: 0, screenHeight: 360 },
			point,
			position: { x: 27, y: -1 },
			rectangle: true,
		};

		expect(SelectionGestureSchema.safeParse(start).success).toBe(true);
		expect(SelectionGestureSchema.safeParse(update).success).toBe(true);
		expect(SelectionGestureSchema.safeParse({ ...identity, action: 'end', point }).success).toBe(
			true,
		);
		expect(SelectionGestureSchema.safeParse({ ...identity, action: 'cancel' }).success).toBe(true);
		expect(SelectionGestureSchema.safeParse({ ...identity, action: 'end' }).success).toBe(false);
		expect(SelectionGestureSchema.safeParse({ ...identity, action: 'cancel', point }).success).toBe(
			false,
		);
		expect(SelectionGestureSchema.safeParse({ ...start, geometry: update.geometry }).success).toBe(
			false,
		);
		expect(SelectionGestureSchema.safeParse({ ...update, clickCount: 2 }).success).toBe(false);
		expect(
			SelectionGestureSchema.safeParse({ ...update, position: { x: 65_536, y: 0 } }).success,
		).toBe(false);
	});

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
		expect(
			ResizeLeaseChangedSchema.parse({
				attachmentId: attachment.attachmentId,
				resizeLease: true,
				terminalId: attachment.terminalId,
			}),
		).toEqual({
			attachmentId: attachment.attachmentId,
			resizeLease: true,
			terminalId: attachment.terminalId,
		});
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

	it('separates visual creation from a geometry-pinned terminal start', () => {
		const terminalId = '22'.repeat(16);
		const identity = {
			creationId: '11'.repeat(16),
			hostEpoch: '33'.repeat(16),
			terminalId,
		};
		const geometry = {
			cellPixelHeight: 18,
			cellPixelWidth: 9,
			columns: 117,
			rows: 38,
		};
		const deferredLaunch = {
			...launch,
			cellPixelHeight: null,
			cellPixelWidth: null,
			columns: null,
			rows: null,
		};

		expect(TerminalLaunchSchema.parse(deferredLaunch)).toEqual(deferredLaunch);
		expect(
			TerminalLaunchSchema.safeParse({ ...deferredLaunch, columns: geometry.columns }).success,
		).toBe(false);
		expect(StartTerminalSchema.parse({ geometry, terminalId })).toEqual({ geometry, terminalId });
		expect(
			TerminalStartAcceptedSchema.parse({ geometry, identity, transition: 'started' }),
		).toEqual({ geometry, identity, transition: 'started' });
		expect(
			TerminalStartAcceptedSchema.parse({ geometry, identity, transition: 'already-started' }),
		).toEqual({ geometry, identity, transition: 'already-started' });
		expect(
			TerminalStartAcceptedSchema.safeParse({
				geometry: { ...geometry, columns: geometry.columns + 1 },
				identity,
				transition: 'already-started',
			}).success,
		).toBe(true);
	});

	it('treats environment names as target-operating-system identifiers', () => {
		// The wire schema admits names that differ only by case, because they
		// are distinct JSON members; the Host decides their target identity.
		expect(
			TerminalLaunchSchema.safeParse({
				...launch,
				environment: [
					{ name: 'Path', value: 'a' },
					{ name: 'PATH', value: 'b' },
				],
			}).success,
		).toBe(true);
		// Two names identical exactly as written are one wire member and rejected.
		expect(
			TerminalLaunchSchema.safeParse({
				...launch,
				environment: [
					{ name: 'Path', value: 'a' },
					{ name: 'Path', value: 'b' },
				],
			}).success,
		).toBe(false);

		// Target-specific preparation applies the OS identity rule.
		expect(canonicalEnvironmentName('Path', 'posix')).toBe('Path');
		expect(canonicalEnvironmentName('Path', 'windows')).toBe('PATH');
		const distinctCase = [
			{ name: 'Path', value: 'a' },
			{ name: 'PATH', value: 'b' },
		];
		expect(prepareLaunchEnvironment(distinctCase, 'posix').ok).toBe(true);
		expect(prepareLaunchEnvironment(distinctCase, 'windows')).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
		const exactDuplicate = [
			{ name: 'Path', value: 'a' },
			{ name: 'Path', value: 'b' },
		];
		expect(prepareLaunchEnvironment(exactDuplicate, 'posix')).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
		expect(prepareLaunchEnvironment(exactDuplicate, 'windows')).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidField, ok: false }),
		);
	});

	it('mirrors native launch environment admission for the declared target', () => {
		// Cross-language parity corpus: the native launch inspector admits the
		// same entries for the same declared target (launch_test.zig), and the
		// CreationId digest folds Windows names identically.
		const corpus = {
			distinctCase: [
				{ name: 'Path', value: 'a' },
				{ name: 'PATH', value: 'b' },
			],
			exactDuplicate: [
				{ name: 'Path', value: 'a' },
				{ name: 'Path', value: 'b' },
			],
			uniqueNames: [
				{ name: 'ALPHA', value: '1' },
				{ name: 'BETA', value: null },
			],
		} as const;
		expect(prepareLaunchEnvironment(corpus.distinctCase, 'posix').ok).toBe(true);
		expect(prepareLaunchEnvironment(corpus.distinctCase, 'windows').ok).toBe(false);
		expect(prepareLaunchEnvironment(corpus.exactDuplicate, 'posix').ok).toBe(false);
		expect(prepareLaunchEnvironment(corpus.exactDuplicate, 'windows').ok).toBe(false);
		expect(prepareLaunchEnvironment(corpus.uniqueNames, 'posix').ok).toBe(true);
		expect(prepareLaunchEnvironment(corpus.uniqueNames, 'windows').ok).toBe(true);
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

	it('rejects a UTF-8 BOM before control JSON without changing plain JSON', () => {
		expect(
			decodeControlJson(new Uint8Array([0xef, 0xbb, 0xbf, 0x7b, 0x7d]), EmptyControlSchema),
		).toEqual(expect.objectContaining({ code: ProtocolErrorCode.invalidJson, ok: false }));
		expect(decodeControlJson(new Uint8Array([0x7b, 0x7d]), EmptyControlSchema)).toEqual({
			ok: true,
			value: {},
		});
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
			'"major":0,"minor":0,"patch":7',
			'"major":0,"minor":0,"minor":0,"patch":7',
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

	it('bounds acknowledged input bytes to the input-byte limit', () => {
		const parse = (acceptedBytes: number) =>
			InputAcceptedSchema.safeParse({
				acceptedBytes,
				attachmentId: '22222222222222222222222222222222',
			});
		expect(parse(1024 * 1024).success).toBe(true);
		expect(parse(1024 * 1024 + 1).success).toBe(false);
		expect(parse(1024 * 1024 + 16).success).toBe(false);
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
		const theme = { operationId: null, revision: '0', terminal: terminalTheme } as const;
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
			theme,
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
		const awaitingStart = {
			...common,
			columns: null,
			exit: null,
			failure: null,
			failureContainment: null,
			phase: 'awaiting-start',
			process: null,
			revision: '0',
			rows: null,
			terminateRequested: false,
		};

		expect(TerminalSummarySchema.parse(awaitingStart)).toEqual(awaitingStart);
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
