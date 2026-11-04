import { PROTOCOL_LIMITS } from '@orchbee/terminal-protocol';
import { describe, expect, it } from 'vitest';

import {
	ProjectedAttachmentBootstrapSchema,
	ProjectedTerminalEventSchema,
} from '../projected-terminal-contract';

const attachmentId = '11'.repeat(16);
const identity = {
	creationId: '22'.repeat(16),
	hostEpoch: '33'.repeat(16),
	terminalId: '44'.repeat(16),
};
const terminalTheme = {
	colorScheme: 'dark' as const,
	defaultBackgroundRgb: 0x1e_1e_1e,
	defaultCursorRgb: null,
	defaultForegroundRgb: 0xd4_d4_d4,
	palette: Array.from({ length: 256 }, (_, index) => index),
};
const theme = { operationId: null, revision: '0', terminal: terminalTheme };
const bootstrap = {
	attachmentId,
	canInput: false,
	canResize: false,
	capabilityRevision: '1',
	...identity,
	theme,
};

describe('projected terminal contract', () => {
	it('bounds optional configured Host default colors independently from its active theme', () => {
		const hostDefaultColors = {
			colorScheme: 'light',
			defaultBackgroundRgb: 0xfcfcfc,
			defaultForegroundRgb: 0x141414,
		};
		const candidate = { ...bootstrap, terminalOutcome: null, hostDefaultColors };
		expect(ProjectedAttachmentBootstrapSchema.parse(candidate).hostDefaultColors).toEqual(
			hostDefaultColors,
		);
		for (const invalid of [
			{ ...hostDefaultColors, defaultBackgroundRgb: 0x1000000 },
			{ ...hostDefaultColors, defaultForegroundRgb: -1 },
			{ ...hostDefaultColors, palette: [] },
		]) {
			expect(
				ProjectedAttachmentBootstrapSchema.safeParse({ ...candidate, hostDefaultColors: invalid })
					.success,
			).toBe(false);
		}
	});
	it('carries a typed attachment-scoped bell event', () => {
		const event = {
			event: { attachmentId, sequence: '9' },
			kind: 'bell',
		};

		expect(ProjectedTerminalEventSchema.parse(event)).toEqual(event);
		expect(
			ProjectedTerminalEventSchema.safeParse({
				...event,
				event: { attachmentId, sequence: '0' },
			}).success,
		).toBe(false);
	});

	it('carries a live resize-lease change without mutating the attachment bootstrap', () => {
		const event = {
			event: { attachmentId, resizeLease: true, terminalId: identity.terminalId },
			kind: 'resize-lease-changed',
		};

		expect(ProjectedTerminalEventSchema.parse(event)).toEqual(event);
		expect(bootstrap.canResize).toBe(false);
	});

	it('carries only a positive Host-committed attachment input capability revision', () => {
		const event = {
			capabilities: {
				attachmentId,
				capabilities: { input: false, read: true, resize: true },
				capabilityRevision: '2',
				resizeLease: false,
				terminalId: identity.terminalId,
			},
			kind: 'attachment-input-capability-changed',
		};

		expect(ProjectedTerminalEventSchema.parse(event)).toEqual(event);
		expect(
			ProjectedTerminalEventSchema.safeParse({
				...event,
				capabilities: { ...event.capabilities, capabilityRevision: '0' },
			}).success,
		).toBe(false);
	});

	it('preserves a typed feature failure correlation through the page boundary', () => {
		const event = {
			failure: {
				code: 'resize-backpressure',
				correlationId: '55'.repeat(16),
				message: 'The Host resize queue is full.',
				operation: 'resize',
				retryable: true,
			},
			kind: 'failure',
		};

		expect(ProjectedTerminalEventSchema.parse(event)).toEqual(event);
		expect(
			ProjectedTerminalEventSchema.safeParse({
				...event,
				failure: { ...event.failure, correlationId: 'not-a-correlation' },
			}).success,
		).toBe(false);
	});

	it('represents a terminal outcome as one identity-bound union', () => {
		const exited = {
			event: {
				exit: { code: 0, kind: 'exit-code' },
				finalRevision: '7',
				identity,
			},
			kind: 'exited',
		};
		const failed = {
			event: {
				failure: { kind: 'pty-read', stage: 'read-output' },
				finalRevision: '8',
				identity,
				process: {
					kind: 'observed',
					result: { code: 1, kind: 'exit-code' },
				},
			},
			kind: 'failed',
		};

		expect(
			ProjectedAttachmentBootstrapSchema.safeParse({ ...bootstrap, terminalOutcome: null }).success,
		).toBe(true);
		expect(
			ProjectedAttachmentBootstrapSchema.safeParse({ ...bootstrap, terminalOutcome: exited })
				.success,
		).toBe(true);
		expect(
			ProjectedAttachmentBootstrapSchema.safeParse({ ...bootstrap, terminalOutcome: failed })
				.success,
		).toBe(true);
		expect(ProjectedAttachmentBootstrapSchema.safeParse(bootstrap).success).toBe(false);
		expect(
			ProjectedAttachmentBootstrapSchema.safeParse({
				...bootstrap,
				terminalExit: exited.event,
				terminalFailure: failed.event,
				terminalOutcome: null,
			}).success,
		).toBe(false);
	});

	it('rejects mutable capabilities or a mismatched identity after a final outcome', () => {
		const terminalOutcome = {
			event: {
				exit: { code: 0, kind: 'exit-code' },
				finalRevision: '7',
				identity,
			},
			kind: 'exited',
		};

		expect(
			ProjectedAttachmentBootstrapSchema.safeParse({
				...bootstrap,
				canInput: true,
				terminalOutcome,
			}).success,
		).toBe(false);
		expect(
			ProjectedAttachmentBootstrapSchema.safeParse({
				...bootstrap,
				terminalOutcome: {
					...terminalOutcome,
					event: {
						...terminalOutcome.event,
						identity: { ...identity, terminalId: '55'.repeat(16) },
					},
				},
			}).success,
		).toBe(false);
	});

	it('keeps local filesystem effects an explicit optional attachment capability', () => {
		expect(
			ProjectedAttachmentBootstrapSchema.parse({ ...bootstrap, terminalOutcome: null }),
		).not.toHaveProperty('localFileEffects');
		expect(
			ProjectedAttachmentBootstrapSchema.parse({
				...bootstrap,
				localFileEffects: true,
				terminalOutcome: null,
			}),
		).toMatchObject({ localFileEffects: true });
	});

	it('carries a correlated runtime theme result and failure', () => {
		const operationId = '55'.repeat(16);
		const event = {
			event: {
				current: {
					operationId,
					revision: '1',
					terminal: { ...terminalTheme, defaultBackgroundRgb: 0xfc_fc_fc },
				},
				operationId,
				terminalId: identity.terminalId,
				transition: 'applied',
			},
			kind: 'terminal-theme-result',
		};
		expect(ProjectedTerminalEventSchema.parse(event)).toEqual(event);
		expect(
			ProjectedTerminalEventSchema.parse({
				failure: { code: 'host-request-outcome-unknown', outcome: 'unknown', retryable: true },
				kind: 'terminal-theme-failed',
				operationId,
			}),
		).toMatchObject({ kind: 'terminal-theme-failed', operationId });
	});

	it('preserves binary frame payloads without JSON representation', () => {
		const frame = new ArrayBuffer(32);
		const parsed = ProjectedTerminalEventSchema.parse({ frame, kind: 'frame' });

		expect(parsed.kind).toBe('frame');
		if (parsed.kind !== 'frame') throw new Error('Expected a projected terminal frame event.');
		expect(parsed.frame).toBe(frame);

		const rows = new Uint8Array(new ArrayBuffer(48), 8, 32);
		const parsedRows = ProjectedTerminalEventSchema.parse({ kind: 'rows', rows });
		expect(parsedRows).toMatchObject({ kind: 'rows', rows });
	});

	it('bounds projected frame and row-window payloads by their visible bytes', () => {
		const maximumFrame = new Uint8Array(
			new ArrayBuffer(PROTOCOL_LIMITS.maxFrameBytes + 16),
			8,
			PROTOCOL_LIMITS.maxFrameBytes,
		);
		const maximumRows = new Uint8Array(
			new ArrayBuffer(PROTOCOL_LIMITS.maxRowWindowBytes + 16),
			8,
			PROTOCOL_LIMITS.maxRowWindowBytes,
		);

		expect(
			ProjectedTerminalEventSchema.safeParse({ frame: maximumFrame, kind: 'frame' }).success,
		).toBe(true);
		expect(
			ProjectedTerminalEventSchema.safeParse({ kind: 'rows', rows: maximumRows }).success,
		).toBe(true);
		expect(
			ProjectedTerminalEventSchema.safeParse({
				frame: new ArrayBuffer(PROTOCOL_LIMITS.maxFrameBytes + 1),
				kind: 'frame',
			}).success,
		).toBe(false);
		expect(
			ProjectedTerminalEventSchema.safeParse({
				kind: 'rows',
				rows: new Uint8Array(PROTOCOL_LIMITS.maxRowWindowBytes + 1),
			}).success,
		).toBe(false);
	});

	it('bounds Browser-owned selection copy content', () => {
		const content = new ArrayBuffer(32);

		expect(
			ProjectedTerminalEventSchema.parse({
				content,
				kind: 'selection-copy-content',
				requestSequence: '1',
			}),
		).toMatchObject({ content, kind: 'selection-copy-content', requestSequence: '1' });
	});
});
