import { describe, expect, it } from 'vitest';
import { resolveTabKeyboardReorder } from '../resolveTabKeyboardReorder';

describe('resolveTabKeyboardReorder', () => {
	it('maps modified horizontal and vertical arrows to relative moves', () => {
		expect(
			resolveTabKeyboardReorder({
				key: 'ArrowLeft',
				commandOrControl: true,
				shift: false,
				horizontal: true,
				mac: false,
			}),
		).toEqual({ direction: -1, moveToEnd: false });
		expect(
			resolveTabKeyboardReorder({
				key: 'ArrowDown',
				commandOrControl: true,
				shift: false,
				horizontal: false,
				mac: false,
			}),
		).toEqual({ direction: 1, moveToEnd: false });
	});

	it('maps Shift-modified moves to the beginning or end', () => {
		expect(
			resolveTabKeyboardReorder({
				key: 'ArrowRight',
				commandOrControl: true,
				shift: true,
				horizontal: true,
				mac: false,
			}),
		).toEqual({ direction: 1, moveToEnd: true });
	});

	it('accepts the macOS line-boundary remap only with Shift', () => {
		expect(
			resolveTabKeyboardReorder({
				key: 'Home',
				commandOrControl: false,
				shift: true,
				horizontal: true,
				mac: true,
			}),
		).toEqual({ direction: -1, moveToEnd: true });
		expect(
			resolveTabKeyboardReorder({
				key: 'End',
				commandOrControl: false,
				shift: true,
				horizontal: false,
				mac: true,
			}),
		).toEqual({ direction: 1, moveToEnd: true });
		expect(
			resolveTabKeyboardReorder({
				key: 'Home',
				commandOrControl: false,
				shift: false,
				horizontal: true,
				mac: true,
			}),
		).toBeNull();
		expect(
			resolveTabKeyboardReorder({
				key: 'End',
				commandOrControl: false,
				shift: true,
				horizontal: true,
				mac: false,
			}),
		).toBeNull();
	});

	it('rejects unrelated or unmodified direction keys', () => {
		expect(
			resolveTabKeyboardReorder({
				key: 'ArrowLeft',
				commandOrControl: false,
				shift: false,
				horizontal: true,
				mac: false,
			}),
		).toBeNull();
		expect(
			resolveTabKeyboardReorder({
				key: 'PageUp',
				commandOrControl: true,
				shift: false,
				horizontal: true,
				mac: false,
			}),
		).toBeNull();
	});
});
