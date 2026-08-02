import { describe, expect, it } from 'vitest';

import {
	CanonicalUint64TextSchema,
	decodeControlJson,
	FrameAppliedSchema,
	InputAcceptedSchema,
	ListTerminalsV1_3Schema,
	NeedSnapshotSchema,
	ResizeTerminalV1_1Schema,
	SetProjectionDemandSchema,
	TerminalLaunchSchema,
	TerminalLaunchV1_1Schema,
	TerminalListV1_3Schema,
} from '../control';
import { ProtocolErrorCode } from '../protocol-result';

const launch = {
	arguments: ['--version'],
	columns: 80,
	currentDirectory: null,
	environment: [
		{ name: 'EMPTY', value: '' },
		{ name: 'REMOVED', value: null },
	],
	executable: '/bin/example',
	rows: 24,
};

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

describe('control schemas', () => {
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
	});

	it('bounds direct schema text by encoded bytes', () => {
		expect(
			TerminalLaunchSchema.safeParse({
				...launch,
				executable: '🐝'.repeat(20_000),
			}).success,
		).toBe(false);
	});

	it('validates canonical u64 text and strict UTF-8 JSON', () => {
		expect(CanonicalUint64TextSchema.safeParse('18446744073709551615').success).toBe(true);
		expect(CanonicalUint64TextSchema.safeParse('01').success).toBe(false);
		expect(
			decodeControlJson(new TextEncoder().encode(JSON.stringify(launch)), TerminalLaunchSchema),
		).toEqual({ ok: true, value: launch });
		expect(decodeControlJson(new Uint8Array([0xff]), TerminalLaunchSchema)).toEqual(
			expect.objectContaining({ code: ProtocolErrorCode.invalidUtf8, ok: false }),
		);
	});

	it('requires resolved emulation and pixel geometry only on the protocol 1.1 launch', () => {
		expect(TerminalLaunchSchema.safeParse(launch).success).toBe(true);
		expect(
			TerminalLaunchV1_1Schema.safeParse({
				...launch,
				cellPixelHeight: 16,
				cellPixelWidth: 8,
				terminalProfile: profile,
			}).success,
		).toBe(true);
		expect(
			TerminalLaunchV1_1Schema.safeParse({
				...launch,
				cellPixelHeight: 16,
				cellPixelWidth: 8,
				columns: 4096,
				rows: 4096,
				terminalProfile: profile,
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
			ResizeTerminalV1_1Schema.safeParse({
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

	it('bounds protocol 1.3 inventory pages without bounding total inventory', () => {
		const request = {
			cursor: {
				inventoryGeneration: '7',
				offset: '128',
			},
			maximumTerminals: 128,
		};
		expect(ListTerminalsV1_3Schema.parse(request)).toEqual(request);
		expect(ListTerminalsV1_3Schema.safeParse({ ...request, maximumTerminals: 129 }).success).toBe(
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
		expect(TerminalListV1_3Schema.parse(response)).toEqual(response);
		expect(
			TerminalListV1_3Schema.safeParse({
				...response,
				nextCursor: { ...response.nextCursor, inventoryGeneration: '8' },
			}).success,
		).toBe(false);
	});
});
