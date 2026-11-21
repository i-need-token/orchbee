import { workbenchMessages } from '@orchbee/i18n';
import { TerminalPresentationIdSchema } from '@orchbee/workbench-client-protocol';
import { describe, expect, it } from 'vitest';

import type { WorkbenchAgentConfirmation } from '../../types/workbench-agent-confirmations';
import { createWorkbenchAgentConfirmationNews } from '../workbench-agent-confirmation-news';

const shown = TerminalPresentationIdSchema.parse('terminal-shown');
const hidden = TerminalPresentationIdSchema.parse('terminal-hidden');

function row(serial: number, stopped = false): WorkbenchAgentConfirmation {
	return {
		agent: 'claude',
		choice: 'run',
		offer: false,
		pressed: serial === 0 ? null : { kind: 'command', serial, text: `command ${serial}` },
		stopped,
		wait: null,
	};
}

describe('Agent confirmation news', () => {
	// "Allowed" tells the person Orchbee just pressed a key for them, so it is never said for a press
	// they cannot see, a press from before this Client looked, or a press said already.
	it('says an answer once, when it is news for a terminal in view', () => {
		const news = createWorkbenchAgentConfirmationNews();
		const inView = (presentationId: string): boolean => presentationId === shown;
		const allowed = (serial: number) => ({
			descriptor: workbenchMessages.agentConfirmationAllowedCommand,
			lifetime: 'timed',
			values: { command: `command ${serial}` },
		});
		const read = (rows: readonly (readonly [typeof shown, WorkbenchAgentConfirmation])[]) =>
			news(new Map(rows), inView);

		expect(
			read([
				[shown, row(3)],
				[hidden, row(7)],
			]),
		).toBeNull();
		expect(
			read([
				[shown, row(4)],
				[hidden, row(7)],
			]),
		).toEqual(allowed(4));
		expect(
			read([
				[shown, row(4)],
				[hidden, row(8)],
			]),
		).toBeNull();

		// Another run in the same terminal counts from the start; its earlier answers were never said.
		expect(
			read([
				[shown, row(1)],
				[hidden, row(8)],
			]),
		).toBeNull();
		expect(
			read([
				[shown, row(2)],
				[hidden, row(8)],
			]),
		).toEqual(allowed(2));

		// Answering that stopped is said wherever its terminal is, once.
		const stopped = { descriptor: workbenchMessages.agentConfirmationStopped, lifetime: 'timed' };
		expect(
			read([
				[shown, row(2)],
				[hidden, row(8, true)],
			]),
		).toEqual(stopped);
		expect(
			read([
				[shown, row(2)],
				[hidden, row(8, true)],
			]),
		).toBeNull();

		// A row that left and returned is read afresh.
		expect(read([[hidden, row(8, true)]])).toBeNull();
		expect(
			read([
				[shown, row(5)],
				[hidden, row(8, true)],
			]),
		).toBeNull();
	});
});
