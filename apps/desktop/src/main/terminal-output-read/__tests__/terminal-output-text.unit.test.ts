import {
	ActiveScreen,
	RowPromptClass,
	type DecodedTerminalTextRow,
} from '@orchbee/terminal-protocol';
import { WORKBENCH_TERMINAL_OUTPUT_MAXIMUM_TEXT_BYTES } from '@orchbee/workbench/semantic-commands';
import { terminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it } from 'vitest';

import { projectTerminalOutputText } from '../terminal-output-text';

describe('Terminal output text projection', () => {
	it('joins soft-wrapped rows while retaining hard row boundaries', () => {
		const output = projectTerminalOutputText({
			activeScreen: ActiveScreen.primary,
			generation: 2n,
			presentationId: terminalPresentationId('presentation-output'),
			requestedRows: 3,
			revision: 7n,
			rows: [
				row(0n, 'hello ', true, false),
				row(1n, 'world', false, true),
				row(2n, 'next', false, false),
			],
			totalRows: 3n,
		});

		expect(output).toEqual({
			activeScreen: 'primary',
			format: 'plain-text',
			generation: '2',
			presentationId: 'presentation-output',
			requestedRows: 3,
			returnedRows: 3,
			revision: '7',
			source: 'tail-unwrapped',
			startRow: '0',
			text: 'hello world\nnext',
			totalRows: '3',
			truncated: false,
		});
	});

	it('keeps a valid UTF-8 suffix when one newest row exceeds the public byte cap', () => {
		const output = projectTerminalOutputText({
			activeScreen: ActiveScreen.alternate,
			generation: 1n,
			presentationId: terminalPresentationId('presentation-large-output'),
			requestedRows: 1,
			revision: 0n,
			rows: [
				row(
					0n,
					'🐝'.repeat(Math.ceil(WORKBENCH_TERMINAL_OUTPUT_MAXIMUM_TEXT_BYTES / 4) + 1),
					false,
					false,
				),
			],
			totalRows: 1n,
		});

		expect(new TextEncoder().encode(output.text).byteLength).toBeLessThanOrEqual(
			WORKBENCH_TERMINAL_OUTPUT_MAXIMUM_TEXT_BYTES,
		);
		expect(output.text.includes('\uFFFD')).toBe(false);
		expect(output).toMatchObject({ returnedRows: 1, startRow: '0', truncated: true });
	});

	it('drops only a contiguous oldest prefix when complete rows exceed the cap', () => {
		const output = projectTerminalOutputText({
			activeScreen: ActiveScreen.primary,
			generation: 1n,
			presentationId: terminalPresentationId('presentation-bounded-output'),
			requestedRows: 2,
			revision: 1n,
			rows: [
				row(8n, 'x'.repeat(WORKBENCH_TERMINAL_OUTPUT_MAXIMUM_TEXT_BYTES), false, false),
				row(9n, 'newest', false, false),
			],
			totalRows: 10n,
		});

		expect(output).toMatchObject({
			returnedRows: 1,
			startRow: '9',
			text: 'newest',
			truncated: true,
		});
	});
});

function row(
	index: bigint,
	text: string,
	wrap: boolean,
	wrapContinuation: boolean,
): DecodedTerminalTextRow {
	return {
		byteOffsets: new Uint32Array(0),
		columns: new Uint16Array(0),
		identityAscii: false,
		index,
		promptClass: RowPromptClass.none,
		text,
		utf16Offsets: new Uint32Array(0),
		widths: new Uint8Array(0),
		wrap,
		wrapContinuation,
	};
}
