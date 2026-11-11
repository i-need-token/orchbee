// @vitest-environment jsdom

import { ActionRuntime, createTypedActionRuntime, defineAction } from '@orchbee/actions';
import { OrchbeeI18nProvider, workbenchMessages } from '@orchbee/i18n';
import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	WORKBENCH_ACTION_IDS,
	type WorkbenchActionCatalog,
	workbenchAgentStartInput,
} from '../../../actions/workbench-action-ids';
import type {
	WorkbenchAgentCatalogSnapshot,
	WorkbenchAgentStartInput,
} from '../../../types/workbench-agent-start';
import { WorkbenchAgentStartDialog } from '../WorkbenchAgentStartDialog';

const CATALOG: WorkbenchAgentCatalogSnapshot = {
	agents: [
		{ command: 'pi', productName: 'Pi', providerId: 'pi', state: 'not-found' },
		{ command: 'claude', productName: 'Claude Code', providerId: 'claude-code', state: 'found' },
		{ command: 'codex', productName: 'Codex', providerId: 'codex', state: 'found' },
	],
	lookup: { checkedAt: 1_794_300_000_000, status: 'complete' },
	recent: ['codex'],
	startable: true,
};

afterEach(cleanup);

describe('WorkbenchAgentStartDialog', () => {
	it('clears a query before Escape closes, and Start runs the start with the highlighted Agent and shown place', async () => {
		const runtime = new ActionRuntime();
		const scope = runtime.createScope('workbench');
		const started = vi.fn<(input: WorkbenchAgentStartInput) => void>();
		createTypedActionRuntime<WorkbenchActionCatalog>(runtime).register(
			scope,
			defineAction({
				category: { id: 'terminal', title: workbenchMessages.actionCategoryTerminal },
				execution: { concurrency: 'parallel', concurrencyKey: () => 'start', mode: 'foreground' },
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: (_context, input) => {
						started(input);
						return { kind: 'completed' };
					},
				},
				id: WORKBENCH_ACTION_IDS.startAgent,
				input: workbenchAgentStartInput,
				title: { defaultMessage: 'Start Agent', id: 'test.start' },
			}),
		);
		const refresh = vi.fn<() => void>();
		const onClose = vi.fn<() => void>();
		render(
			<OrchbeeI18nProvider locale='en-US'>
				<WorkbenchAgentStartDialog
					catalog={{ getSnapshot: () => CATALOG, refresh, subscribe: () => () => undefined }}
					finalFocus={() => null}
					places={['new-tab', 'split-right']}
					onClose={onClose}
					onStart={(input) =>
						runtime.execute(WORKBENCH_ACTION_IDS.startAgent, input, { scope, source: 'button' })
					}
				/>
			</OrchbeeI18nProvider>,
		);
		const search = screen.getByRole('combobox', { name: 'Search' });
		await waitFor(() => expect(document.activeElement).toBe(search));
		expect(refresh).toHaveBeenCalledOnce();

		fireEvent.change(search, { target: { value: 'claude' } });
		expect(screen.queryByRole('option', { name: /Codex/u })).toBeNull();
		fireEvent.keyDown(search, { key: 'Escape' });
		expect(search).toHaveProperty('value', '');
		expect(onClose).not.toHaveBeenCalled();
		fireEvent.keyDown(search, { key: 'Escape' });
		await waitFor(() => expect(onClose).toHaveBeenCalledOnce());

		// Recent Codex is highlighted first; Down reaches Installed Claude Code.
		fireEvent.keyDown(search, { key: 'ArrowDown' });
		expect(screen.getByRole('option', { name: /Claude Code/u }).getAttribute('aria-selected')).toBe(
			'true',
		);
		fireEvent.change(screen.getByLabelText('Opens in'), { target: { value: 'split-right' } });
		fireEvent.click(screen.getByRole('button', { name: /Start/u }));
		await waitFor(() =>
			expect(started).toHaveBeenCalledWith({ place: 'split-right', providerId: 'claude-code' }),
		);
	});
});
