import { beforeAll, describe, expect, it } from 'vitest';
import { encodePaste, inspectPaste } from '../paste-policy';
import type { WasmBridge } from '../../wasm-bridge';
import { initializeWasm } from '../../__tests__/test-helpers';

let bridge: WasmBridge;

beforeAll(async () => {
	bridge = await initializeWasm();
});

describe('paste policy', () => {
	it('replaces terminal control bytes', () => {
		const inspection = inspectPaste('hel\x1blo\0\x03world', { bracketed: true }, bridge);
		expect(inspection.sanitized).toBe('hel lo  world');
		expect(inspection.replacedControlBytes).toBe(3);
	});

	it('converts newlines only for non-bracketed paste', () => {
		const plain = inspectPaste(
			'first\nsecond',
			{
				bracketed: false,
				allowUnsafe: true,
			},
			bridge,
		);
		const bracketed = inspectPaste('first\nsecond', { bracketed: true }, bridge);
		expect(encodePaste(plain, bridge)).toBe('first\rsecond');
		expect(encodePaste(bracketed, bridge)).toBe('\x1b[200~first\nsecond\x1b[201~');
	});

	it('requires confirmation for multiline non-bracketed paste', () => {
		expect(inspectPaste('first\nsecond', { bracketed: false }, bridge).classification).toBe(
			'confirm',
		);
		expect(inspectPaste('first\nsecond', { bracketed: true }, bridge).classification).toBe('safe');
	});

	it('never trusts an embedded bracketed-paste terminator', () => {
		const inspection = inspectPaste(`safe\x1b[201~unsafe`, { bracketed: true }, bridge);
		expect(inspection.embeddedBracketTerminator).toBe(true);
		expect(inspection.classification).toBe('confirm');
		expect(inspection.sanitized).toBe('safe [201~unsafe');
	});

	it('supports disabling protection without disabling sanitization', () => {
		const inspection = inspectPaste(
			'first\n\x1bsecond',
			{
				bracketed: false,
				protection: false,
			},
			bridge,
		);
		expect(inspection.classification).toBe('safe');
		expect(inspection.sanitized).toBe('first\n second');
	});

	it.each([
		['Cyrillic', '/Users/test/Пример/Output'],
		['Chinese', '/Users/test/文件夹/file'],
		['Japanese', '/Users/test/フォルダ/file'],
		['Arabic', '/Users/test/مجلد/file'],
		['Hebrew', '/Users/test/תיקייה/file'],
		['Thai', 'กิ'],
		['emoji', 'folder📁name'],
		['ZWJ emoji', 'test👨‍👩‍👧file'],
		['Latin Extended', '/Users/test/café/naïve'],
		['combining marks', 'e\u0301 a\u0300\u0303'],
		['non-breaking space', 'hello\u00a0world'],
		['private-use character', 'test\ue000char'],
	])('preserves $0 text during paste sanitization', (_name, text) => {
		const inspection = inspectPaste(text, { bracketed: true }, bridge);
		expect(inspection.sanitized).toBe(text);
		expect(inspection.replacedControlBytes).toBe(0);
		expect(encodePaste(inspection, bridge)).toBe(`\x1b[200~${text}\x1b[201~`);
	});

	it.each([
		['NUL', '\u0000'],
		['start of heading', '\u0001'],
		['end of text', '\u0003'],
		['bell', '\u0007'],
		['Ctrl-V', '\u0016'],
		['delete', '\u007f'],
		['C1 next-line', '\u0085'],
	])('replaces the unsafe $0 control byte', (_name, control) => {
		const inspection = inspectPaste(`before${control}after`, { bracketed: true }, bridge);
		expect(inspection.sanitized).toBe('before after');
		expect(inspection.replacedControlBytes).toBe(1);
	});
});
