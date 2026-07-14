import { beforeAll, describe, expect, it } from 'vitest';
import { GhosttyTerminal } from '../ghostty-terminal';
import { DirtyState } from '../types';
import { createTerminal, initializeWasm } from './test-helpers';

beforeAll(initializeWasm);

describe('GhosttyTerminal render adapter', () => {
	it('captures immutable lines, cursor state, and dimensions', () => {
		const terminal = createTerminal(8, 2);
		const adapter = new GhosttyTerminal(terminal);
		terminal.write('Hi');
		terminal.grid.update();
		adapter.refresh(DirtyState.FULL);
		expect(adapter.hasFrame).toBe(true);
		expect(adapter.getDimensions()).toEqual({ cols: 8, rows: 2 });
		expect(adapter.getLine(0)?.[0]?.codepoint).toBe(72);
		expect(adapter.getGraphemeString(0, 1)).toBe('i');
		expect(adapter.getCursor()).toMatchObject({ x: 2, y: 0, visible: true });
		terminal.dispose();
	});

	it('clears dirty state after a captured frame', () => {
		const terminal = createTerminal();
		const adapter = new GhosttyTerminal(terminal);
		terminal.write('dirty');
		terminal.grid.update();
		adapter.refresh(DirtyState.PARTIAL);
		adapter.clearDirty();
		expect(adapter.needsFullRedraw()).toBe(false);
		expect(adapter.isRowDirty(0)).toBe(false);
		terminal.dispose();
	});
});
