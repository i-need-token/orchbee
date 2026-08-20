// @vitest-environment jsdom

import { ActionRuntime, defineAction, type ActionExecutionContext } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import { act, cleanup, renderHook, waitFor } from '@testing-library/react';
import * as React from 'react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import type { WorkbenchTerminalActionContextProvider } from '../../../../../../../../../../types/workbench-terminal-surface';
import { useWorkbenchTerminalActions } from '../useWorkbenchTerminalActions';

afterEach(cleanup);

describe('useWorkbenchTerminalActions', () => {
	it('pins product context and executes the prepared invocation through the surface provider', async () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('test.workbench');
		const executed =
			vi.fn<
				(value: {
					readonly context: ActionExecutionContext;
					readonly input: { readonly url: string };
				}) => void
			>();
		const wrapper = ({ children }: React.PropsWithChildren) =>
			React.createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children);
		const rendered = renderHook(
			() =>
				useWorkbenchTerminalActions({
					parentScope,
					presentationId: 'presentation-1',
					runtime,
				}),
			{ wrapper },
		);
		const registration = runtime.register(
			rendered.result.current.scope,
			defineAction({
				category: {
					id: 'terminal',
					title: { defaultMessage: 'Terminal', id: 'test.terminal.category' },
				},
				execution: {
					concurrency: 'parallel',
					concurrencyKey: () => 'test-terminal-link',
					mode: 'foreground',
				},
				getAvailability: () => ({ kind: 'enabled' }),
				handler: {
					execute: (context, input: { readonly url: string }) => {
						executed({ context, input });
						context.markCommitted();
						return { kind: 'completed' };
					},
				},
				id: 'terminal.link.open',
				input: {
					name: 'test-link',
					parse: (input) =>
						typeof input === 'object' &&
						input !== null &&
						typeof Reflect.get(input, 'url') === 'string'
							? { data: { url: String(Reflect.get(input, 'url')) }, success: true }
							: { code: 'invalid-link', success: false },
				},
				title: { defaultMessage: 'Open terminal link', id: 'test.terminal.link.open' },
			}),
		);
		const capturedInput = { url: 'https://orchbee.dev/original' };
		const provider: WorkbenchTerminalActionContextProvider = {
			capture: () => ({
				actionInputs: { 'terminal.link.open': capturedInput },
				focus: { terminal: true },
				kind: 'product',
				target: { id: 'attachment-1', type: 'terminal-attachment' },
			}),
			executePrepared: vi.fn<WorkbenchTerminalActionContextProvider['executePrepared']>(
				(invocation) => runtime.executePrepared(invocation),
			),
		};
		let providerRegistration:
			| ReturnType<typeof rendered.result.current.bindActionContextProvider>
			| undefined;
		act(() => {
			providerRegistration = rendered.result.current.bindActionContextProvider(provider);
		});
		act(() => {
			expect(rendered.result.current.captureContext(contextMenuEvent())).toBe(true);
		});

		await waitFor(() =>
			expect(rendered.result.current.entries.map((entry) => entry.label)).toContain(
				'Open terminal link',
			),
		);
		capturedInput.url = 'https://orchbee.dev/changed';
		const linkEntry = rendered.result.current.entries.find(
			(entry) => entry.id === 'terminal-action:terminal.link.open',
		);
		if (linkEntry === undefined) throw new Error('Expected the pinned link action.');
		act(() => linkEntry.onSelect({} as React.MouseEvent<HTMLElement>));

		await waitFor(() => expect(executed).toHaveBeenCalledOnce());
		expect(executed).toHaveBeenCalledWith({
			context: expect.objectContaining({
				focus: { terminal: true },
				source: 'context-menu',
				target: { id: 'attachment-1', type: 'terminal-attachment' },
			}),
			input: { url: 'https://orchbee.dev/original' },
		});
		expect(provider.executePrepared).toHaveBeenCalledOnce();
		if (providerRegistration === undefined) throw new Error('Expected a provider registration.');
		providerRegistration.dispose();
		registration.dispose();
		runtime.dispose();
	});

	it('suppresses the product menu when the terminal application owns the pointer context', () => {
		const runtime = new ActionRuntime();
		const parentScope = runtime.createScope('test.workbench');
		const wrapper = ({ children }: React.PropsWithChildren) =>
			React.createElement(OrchbeeI18nProvider, { locale: 'en-US' }, children);
		const rendered = renderHook(
			() =>
				useWorkbenchTerminalActions({
					parentScope,
					presentationId: 'presentation-1',
					runtime,
				}),
			{ wrapper },
		);
		const providerRegistration = rendered.result.current.bindActionContextProvider({
			capture: () => ({ kind: 'terminal-owned' }),
			executePrepared: (invocation) => runtime.executePrepared(invocation),
		});

		act(() => {
			expect(rendered.result.current.captureContext(contextMenuEvent())).toBe(false);
		});
		expect(rendered.result.current.entries).toEqual([]);
		providerRegistration.dispose();
		runtime.dispose();
	});
});

function contextMenuEvent(): React.MouseEvent<HTMLElement> {
	return {
		altKey: false,
		clientX: 24,
		clientY: 48,
		ctrlKey: false,
		metaKey: false,
		shiftKey: true,
	} as React.MouseEvent<HTMLElement>;
}
