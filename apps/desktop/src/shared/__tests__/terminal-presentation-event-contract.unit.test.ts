import {
	terminalPresentationId,
	terminalPresentationOperationId,
} from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it } from 'vitest';

import { DesktopTerminalPresentationEventSchema } from '../terminal-presentation-event-contract';

describe('DesktopTerminalPresentationEventSchema', () => {
	it('accepts every Main-owned observation kind used by a projected client', () => {
		const operationId = terminalPresentationOperationId('operation-1');
		const presentationId = terminalPresentationId('presentation-1');
		const events = [
			{ operationId, presentationId, type: 'terminal-provision-completed' },
			{ operationId, presentationId, type: 'terminal-restore-started' },
			{ operationId, presentationId, type: 'terminal-restore-completed' },
			{ operationId, presentationId, type: 'terminal-close-completed' },
			{ exitCode: 0, presentationId, signal: null, type: 'terminal-exited' },
		] as const;

		expect(
			events.every((event) => DesktopTerminalPresentationEventSchema.safeParse(event).success),
		).toBe(true);
	});

	it('rejects ambiguous exits and renderer command messages', () => {
		expect(
			DesktopTerminalPresentationEventSchema.safeParse({
				exitCode: 1,
				presentationId: 'presentation-1',
				signal: 'SIGTERM',
				type: 'terminal-exited',
			}).success,
		).toBe(false);
		expect(
			DesktopTerminalPresentationEventSchema.safeParse({
				operationId: 'operation-1',
				presentationId: 'presentation-1',
				type: 'terminal-provision-requested',
			}).success,
		).toBe(false);
	});
});
