import { beforeAll, describe, expect, it } from 'vitest';
import { createTerminal, initializeWasm } from './test-helpers';

beforeAll(initializeWasm);

describe('Buffer API', () => {
	it('exposes active viewport lines and cells', () => {
		const terminal = createTerminal(8, 2);
		terminal.write('one\r\ntwo');
		expect(terminal.buffer.active.getLine(0)?.translateToString(true)).toBe('one');
		expect(terminal.buffer.active.getLine(1)?.getCell(0)?.getChars()).toBe('t');
		terminal.dispose();
	});

	it('tracks normal and alternate buffers', () => {
		const terminal = createTerminal();
		expect(terminal.buffer.active.type).toBe('normal');
		terminal.write('\x1b[?1049h');
		expect(terminal.buffer.active.type).toBe('alternate');
		terminal.dispose();
	});

	it('reports xterm-compatible scrollback offsets', () => {
		const terminal = createTerminal(8, 2, { scrollbackLimit: 10 });
		terminal.write('one\r\ntwo\r\nthree');
		expect(terminal.buffer.active.length).toBe(3);
		expect(terminal.buffer.active.baseY).toBe(1);
		terminal.scrollToTop();
		expect(terminal.buffer.active.viewportY).toBe(0);
		terminal.dispose();
	});
});
