// @vitest-environment jsdom

import type {
	ProjectedTerminalInjectedInput,
	ProjectedTerminalInjectedInputReadiness,
	ProjectedTerminalInjectedInputResult,
} from '@orchbee/terminal';
import { describe, expect, it, vi } from 'vitest';

import {
	MobileTerminalInputController,
	type MobileTerminalInputDriver,
} from '../MobileTerminalInputController';

describe('MobileTerminalInputController', () => {
	it('captures one printable ASCII input with the armed modifiers and pinned revision', async () => {
		const fixture = createFixture();
		const controller = new MobileTerminalInputController(fixture.root, fixture.driver);
		controller.toggleModifier('control');
		controller.toggleModifier('alt');
		const event = new InputEvent('beforeinput', {
			bubbles: true,
			cancelable: true,
			data: 'A',
			inputType: 'insertText',
		});

		fixture.textarea.dispatchEvent(event);
		await Promise.resolve();

		expect(event.defaultPrevented).toBe(true);
		expect(fixture.inject).toHaveBeenCalledWith(
			expect.objectContaining({
				code: 'KeyA',
				key: 'A',
				kind: 'key',
				modifiers: expect.objectContaining({ alt: true, control: true, shift: true }),
			}),
			'attachment:1',
		);
		expect(controller.state).toMatchObject({ alt: false, control: false });
		controller.dispose();
	});

	it('consumes toolbar and physical keys before blocked publication', async () => {
		const fixture = createFixture({ kind: 'blocked' });
		const controller = new MobileTerminalInputController(fixture.root, fixture.driver);
		controller.toggleModifier('control');
		expect(controller.sendKey('Escape')).toBe(true);
		expect(controller.state.control).toBe(false);

		controller.toggleModifier('alt');
		fixture.textarea.dispatchEvent(
			new KeyboardEvent('keydown', {
				bubbles: true,
				cancelable: true,
				code: 'ArrowLeft',
				key: 'ArrowLeft',
			}),
		);
		await Promise.resolve();

		expect(controller.state.alt).toBe(false);
		expect(fixture.inject.mock.calls.map(([, revision]) => revision)).toEqual([
			'attachment:1',
			'attachment:1',
		]);
		expect(fixture.inject.mock.calls[1]?.[0]).toMatchObject({
			code: 'ArrowLeft',
			modifiers: expect.objectContaining({ alt: true }),
		});
		controller.dispose();
	});

	it('clears modifiers for composition, paste, non-ASCII commits, blur, and replacement', () => {
		const fixture = createFixture();
		const controller = new MobileTerminalInputController(fixture.root, fixture.driver);
		const clearWith = (event: Event): void => {
			controller.toggleModifier('control');
			fixture.textarea.dispatchEvent(event);
			expect(controller.state.control).toBe(false);
		};

		clearWith(new CompositionEvent('compositionstart', { bubbles: true }));
		clearWith(new Event('paste', { bubbles: true }));
		clearWith(
			new InputEvent('beforeinput', {
				bubbles: true,
				cancelable: true,
				data: '中',
				inputType: 'insertText',
			}),
		);
		controller.toggleModifier('alt');
		fixture.textarea.dispatchEvent(new FocusEvent('blur'));
		expect(controller.state.alt).toBe(false);

		controller.toggleModifier('control');
		fixture.setReadiness({ kind: 'ready', revision: 'attachment:2' });
		expect(controller.state).toMatchObject({ control: false });
		controller.toggleModifier('alt');
		fixture.setReadiness({ kind: 'unavailable' });
		expect(controller.state).toMatchObject({ alt: false, readiness: { kind: 'unavailable' } });
		expect(fixture.inject).not.toHaveBeenCalled();
		controller.dispose();
	});

	it('reflects actual textarea focus while toggling the native keyboard client', () => {
		const fixture = createFixture();
		const controller = new MobileTerminalInputController(fixture.root, fixture.driver);

		expect(controller.toggleKeyboard()).toBe(true);
		expect(document.activeElement).toBe(fixture.textarea);
		expect(controller.state.focused).toBe(true);
		expect(controller.toggleKeyboard()).toBe(true);
		expect(document.activeElement).not.toBe(fixture.textarea);
		expect(controller.state.focused).toBe(false);
		controller.dispose();
	});
});

function createFixture(result: ProjectedTerminalInjectedInputResult = { kind: 'sent' }) {
	const root = document.createElement('div');
	const textarea = document.createElement('textarea');
	root.append(textarea);
	document.body.append(root);
	let readiness: ProjectedTerminalInjectedInputReadiness = {
		kind: 'ready',
		revision: 'attachment:1',
	};
	const readinessListeners = new Set<(event: ProjectedTerminalInjectedInputReadiness) => unknown>();
	const inject = vi.fn<
		(
			input: ProjectedTerminalInjectedInput,
			revision: string,
		) => Promise<ProjectedTerminalInjectedInputResult>
	>(
		async (
			_input: ProjectedTerminalInjectedInput,
			_revision: string,
		): Promise<ProjectedTerminalInjectedInputResult> => result,
	);
	const driver = {
		blur: () => textarea.blur(),
		focusMobileInput: () => {
			textarea.focus();
			return true;
		},
		injectInput: inject,
		get injectedInputReadiness() {
			return readiness;
		},
		onInjectedInputReadinessChange: (
			listener: (event: ProjectedTerminalInjectedInputReadiness) => unknown,
		) => {
			readinessListeners.add(listener);
			return {
				dispose: () => {
					readinessListeners.delete(listener);
				},
			};
		},
		textarea,
	} satisfies MobileTerminalInputDriver;
	return {
		driver,
		inject,
		root,
		setReadiness: (next: ProjectedTerminalInjectedInputReadiness) => {
			readiness = next;
			for (const listener of readinessListeners) listener(next);
		},
		textarea,
	};
}
