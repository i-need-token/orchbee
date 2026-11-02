// @vitest-environment jsdom

import { cleanup, renderHook } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import { useTerminalHandoffKeystrokes } from '../useTerminalHandoffKeystrokes';

const insertText = vi.fn<(command: string, showUi: boolean, value: string) => boolean>();

function press(type: 'keydown' | 'keyup', code: string, key: string): KeyboardEvent {
	const event = new KeyboardEvent(type, { bubbles: true, cancelable: true, code, key });
	document.body.dispatchEvent(event);
	return event;
}

/** A terminal that takes focus, consumes letters as the real one does, and records what it gets. */
function focusedTerminal(): string[] {
	const viewport = document.createElement('div');
	viewport.dataset['slot'] = 'workbench-terminal-viewport';
	const input = document.createElement('textarea');
	viewport.append(input);
	document.body.append(viewport);
	const received: string[] = [];
	const receive = (event: KeyboardEvent): void => {
		received.push(`${event.type} ${event.key}`);
		if (/^[a-z]$/u.test(event.key)) event.preventDefault();
	};
	input.addEventListener('keydown', receive);
	input.addEventListener('keyup', receive);
	input.focus();
	return received;
}

function consumeShortcut(event: KeyboardEvent): void {
	event.preventDefault();
}

function handoff() {
	return renderHook(({ settled }) => useTerminalHandoffKeystrokes(settled), {
		initialProps: { settled: false },
	});
}

beforeEach(() => {
	insertText.mockReset();
	document.execCommand = insertText;
});

afterEach(() => {
	cleanup();
	vi.restoreAllMocks();
	document.body.replaceChildren();
});

describe('useTerminalHandoffKeystrokes', () => {
	it('gives the terminal that takes the keyboard what was typed, in order', () => {
		const rendered = handoff();
		press('keydown', 'KeyA', 'a');
		press('keyup', 'KeyA', 'a');
		press('keydown', 'Digit1', '1');
		const tab = press('keydown', 'Tab', 'Tab');

		const received = focusedTerminal();
		rendered.rerender({ settled: true });

		expect(tab.defaultPrevented).toBe(true);
		expect(received).toEqual(['keydown a', 'keyup a', 'keydown 1', 'keydown Tab']);
		// Only the key the terminal left to the browser, and only a key that inserts text.
		expect(insertText.mock.calls).toEqual([['insertText', false, '1']]);
	});

	it('holds a key for an inert layer that has not lost focus yet', () => {
		const rendered = handoff();
		const inertLayer = document.createElement('div');
		inertLayer.setAttribute('inert', '');
		const outgoing = document.createElement('textarea');
		inertLayer.append(outgoing);
		document.body.append(inertLayer);
		outgoing.dispatchEvent(
			new KeyboardEvent('keydown', { bubbles: true, cancelable: true, code: 'KeyA', key: 'a' }),
		);

		const received = focusedTerminal();
		rendered.rerender({ settled: true });

		expect(received).toEqual(['keydown a']);
	});

	it('leaves out what a shortcut used and the release of a key pressed before', () => {
		const rendered = handoff();
		document.addEventListener('keydown', consumeShortcut, { capture: true, once: true });
		press('keydown', 'KeyD', 'D');
		press('keyup', 'ShiftLeft', 'Shift');

		const received = focusedTerminal();
		rendered.rerender({ settled: true });

		expect(received).toEqual([]);
	});

	it('drops everything when the keyboard did not come back to a terminal', () => {
		const rendered = handoff();
		press('keydown', 'KeyA', 'a');
		const button = document.createElement('button');
		const received: string[] = [];
		button.addEventListener('keydown', (event) => received.push(event.key));
		document.body.append(button);
		button.focus();

		rendered.rerender({ settled: true });

		expect(received).toEqual([]);
	});

	it('drops everything when the handoff outlasted typing ahead', () => {
		let clock = 0;
		vi.spyOn(performance, 'now').mockImplementation(() => clock);
		const rendered = handoff();
		press('keydown', 'KeyA', 'a');
		clock += 1_001;
		press('keydown', 'KeyB', 'b');

		const received = focusedTerminal();
		rendered.rerender({ settled: true });

		expect(received).toEqual([]);
	});

	it('stops holding once the handoff is over', () => {
		const rendered = handoff();
		rendered.rerender({ settled: true });

		expect(press('keydown', 'Tab', 'Tab').defaultPrevented).toBe(false);
	});
});
