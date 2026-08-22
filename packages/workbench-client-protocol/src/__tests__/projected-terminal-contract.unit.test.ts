import { PROTOCOL_LIMITS } from '@orchbee/terminal-protocol';
import { describe, expect, it } from 'vitest';

import {
	ProjectedAttachmentBootstrapSchema,
	ProjectedTerminalCommandSchema,
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

	it('keeps commands scoped to their attachment identity', () => {
		for (const kind of ['clear-terminal', 'reset-terminal'] as const) {
			expect(
				ProjectedTerminalCommandSchema.safeParse({
					kind,
					payload: { attachmentId },
				}).success,
			).toBe(true);
			expect(ProjectedTerminalCommandSchema.safeParse({ kind, payload: {} }).success).toBe(false);
		}
		expect(
			ProjectedTerminalCommandSchema.safeParse({
				kind: 'selection-copy',
				payload: { attachmentId, requestSequence: '1' },
			}).success,
		).toBe(true);
		expect(
			ProjectedTerminalCommandSchema.safeParse({
				kind: 'selection-copy',
				payload: { requestSequence: '1' },
			}).success,
		).toBe(false);
		expect(
			ProjectedTerminalCommandSchema.safeParse({
				kind: 'selection-copy',
				payload: { attachmentId, format: 'html', requestSequence: '1' },
			}).success,
		).toBe(false);

		const clipboardDecision = {
			kind: 'clipboard-write-decision',
			payload: {
				approved: true,
				attachmentId,
				sequence: '1',
				text: 'reviewed terminal text',
			},
		};
		expect(ProjectedTerminalCommandSchema.safeParse(clipboardDecision).success).toBe(true);
		expect(
			ProjectedTerminalCommandSchema.safeParse({
				...clipboardDecision,
				payload: { ...clipboardDecision.payload, approved: undefined },
			}).success,
		).toBe(false);
		expect(
			ProjectedTerminalCommandSchema.safeParse({
				...clipboardDecision,
				payload: { ...clipboardDecision.payload, sequence: '0' },
			}).success,
		).toBe(false);
		expect(
			ProjectedTerminalCommandSchema.safeParse({
				...clipboardDecision,
				payload: {
					...clipboardDecision.payload,
					text: 'x'.repeat(PROTOCOL_LIMITS.maxClipboardTextBytes + 1),
				},
			}).success,
		).toBe(false);
	});

	it('carries one attachment-scoped runtime theme operation and its correlated result', () => {
		const operationId = '55'.repeat(16);
		const command = {
			kind: 'apply-terminal-theme',
			payload: {
				attachmentId,
				expectedThemeRevision: '0',
				operationId,
				terminal: { ...terminalTheme, defaultBackgroundRgb: 0xfc_fc_fc },
				terminalId: identity.terminalId,
			},
		};
		expect(ProjectedTerminalCommandSchema.parse(command)).toEqual(command);
		expect(
			ProjectedTerminalCommandSchema.safeParse({
				...command,
				payload: { ...command.payload, attachmentId: undefined },
			}).success,
		).toBe(false);

		const event = {
			event: {
				current: {
					operationId,
					revision: '1',
					terminal: command.payload.terminal,
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

	it('carries explicit accessible selection ranges without reconstructing a pointer gesture', () => {
		expect(
			ProjectedTerminalCommandSchema.safeParse({
				kind: 'selection-command',
				payload: {
					attachmentId,
					command: 'set-range',
					end: { column: 5, row: '8' },
					generation: '2',
					rectangle: false,
					revision: '3',
					start: { column: 1, row: '7' },
				},
			}).success,
		).toBe(true);
	});

	it('bounds link targets by their UTF-8 representation', () => {
		const prefix = 'https://example.com/';
		const exactAsciiTarget = `${prefix}${'a'.repeat(
			PROTOCOL_LIMITS.maxStringBytes - prefix.length,
		)}`;
		const target = `https://example.com/${'界'.repeat(
			Math.ceil(PROTOCOL_LIMITS.maxStringBytes / 3),
		)}`;
		expect(new TextEncoder().encode(exactAsciiTarget).byteLength).toBe(
			PROTOCOL_LIMITS.maxStringBytes,
		);
		expect(
			ProjectedTerminalCommandSchema.safeParse({
				kind: 'link-open',
				payload: { attachmentId, requestSequence: '1', target: exactAsciiTarget },
			}).success,
		).toBe(true);
		expect(
			ProjectedTerminalCommandSchema.safeParse({
				kind: 'link-open',
				payload: { attachmentId, requestSequence: '1', target: `${exactAsciiTarget}a` },
			}).success,
		).toBe(false);
		expect(target.length).toBeLessThan(PROTOCOL_LIMITS.maxStringBytes);
		expect(new TextEncoder().encode(target).byteLength).toBeGreaterThan(
			PROTOCOL_LIMITS.maxStringBytes,
		);
		expect(
			ProjectedTerminalCommandSchema.safeParse({
				kind: 'link-open',
				payload: { attachmentId, requestSequence: '1', target },
			}).success,
		).toBe(false);
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
