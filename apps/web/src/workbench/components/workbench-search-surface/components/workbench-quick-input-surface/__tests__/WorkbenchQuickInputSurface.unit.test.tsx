// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { act, cleanup, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, describe, expect, it } from 'vitest';

import { WorkbenchQuickInputSessions } from '../../../model/WorkbenchQuickInputSessions';
import { WorkbenchQuickInputSurface } from '../WorkbenchQuickInputSurface';

afterEach(cleanup);

function renderSurface(): WorkbenchQuickInputSessions {
	const sessions = new WorkbenchQuickInputSessions();
	render(
		<OrchbeeI18nProvider locale='en-US'>
			<WorkbenchQuickInputSurface sessions={sessions} />
		</OrchbeeI18nProvider>,
	);
	return sessions;
}

describe('WorkbenchQuickInputSurface', () => {
	it('filters a pick by label and accepts the highlighted item with Enter', async () => {
		const sessions = renderSurface();
		let outcome: Promise<unknown> = Promise.resolve();
		act(() => {
			outcome = sessions.pick({
				entries: [
					{ id: 'utf8', kind: 'item', label: 'UTF-8', value: 'utf8' },
					{ kind: 'separator', label: 'More' },
					{ id: 'utf16le', kind: 'item', label: 'UTF-16 LE', value: 'utf16le' },
					{ id: 'utf16be', kind: 'item', label: 'UTF-16 BE', value: 'utf16be' },
				],
				title: 'Reopen with Encoding',
			});
		});
		const input = screen.getByRole('combobox', { name: 'Reopen with Encoding' });
		fireEvent.change(input, { target: { value: '16' } });
		expect(screen.queryByText('More')).toBeNull();
		fireEvent.keyDown(input, { key: 'ArrowDown' });
		fireEvent.keyDown(input, { key: 'Enter' });
		await expect(outcome).resolves.toEqual({ kind: 'accepted', value: 'utf16be' });
		expect(screen.queryByRole('combobox')).toBeNull();
	});

	it('masks a prompt, keeps Enter from accepting a refused value and ends with Escape as cancelled', async () => {
		const sessions = renderSurface();
		let outcome: Promise<unknown> = Promise.resolve();
		act(() => {
			outcome = sessions.input({
				ignoreFocusOut: true,
				password: true,
				prompt: 'Passphrase for the key',
				validate: async (value) =>
					value === '' ? { message: 'Enter a passphrase', severity: 'error' } : null,
			});
		});
		const input = screen.getByLabelText('Quick input', { selector: 'input' });
		expect(input.getAttribute('type')).toBe('password');
		expect(await screen.findByRole('alert')).toHaveProperty('textContent', 'Enter a passphrase');
		fireEvent.keyDown(input, { key: 'Enter' });
		expect(screen.getByText('Passphrase for the key')).toBeTruthy();
		fireEvent.keyDown(input, { key: 'Escape' });
		await expect(outcome).resolves.toEqual({ kind: 'cancelled' });
	});

	it('gives the keys back to the element that held them unless the focus has moved on', () => {
		const sessions = renderSurface();
		const invoker = document.body.appendChild(document.createElement('button'));
		const result = document.body.appendChild(document.createElement('button'));
		const entries = [{ id: 'a', kind: 'item', label: 'a.ts', value: 'a' }] as const;
		invoker.focus();
		act(() => {
			void sessions.pick({ entries });
		});
		const first = screen.getByRole('combobox');
		first.focus();
		// A request that replaces the open one keeps the element the first one found.
		act(() => {
			void sessions.pick({ entries });
		});
		const second = screen.getByRole('combobox');
		second.focus();
		act(() => {
			fireEvent.keyDown(second, { key: 'Escape' });
		});
		expect(document.activeElement).toBe(invoker);
		act(() => {
			void sessions.pick({ entries });
		});
		screen.getByRole('combobox').focus();
		result.focus();
		act(() => {
			sessions.close();
		});
		expect(document.activeElement).toBe(result);
		invoker.remove();
		result.remove();
	});
});
