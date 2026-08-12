// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import {
	AttachmentIdSchema,
	CreationIdSchema,
	HostEpochSchema,
	TerminalIdSchema,
} from '@orchbee/terminal-protocol';
import { terminalPresentationId, type WorkbenchTerminalPresentation } from '@orchbee/workbench';
import { act, createElement, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import type {
	ProjectedTerminalTransport,
	TerminalProjectionClient,
} from '../../../terminal-projection';
import {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
	type ProjectedTerminalRendererSession,
	type startProjectedTerminalRenderer,
} from '../../../projected-terminal-renderer';
import { ProjectedTerminalSurfaceCapabilityContext } from '../../contexts/projected-terminal-surface-capability';
import { useProjectedTerminalSurface } from '../useProjectedTerminalSurface';

const renderer = vi.hoisted(() => ({
	start: vi.fn<typeof startProjectedTerminalRenderer>(),
}));

vi.mock('../../../projected-terminal-renderer', async (importOriginal) => ({
	...(await importOriginal()),
	startProjectedTerminalRenderer: renderer.start,
}));

beforeAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: true,
	});
});

afterAll(() => {
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: false,
	});
});

function TestHarness({
	presentation,
}: {
	readonly presentation: WorkbenchTerminalPresentation;
}): ReactNode {
	const view = useProjectedTerminalSurface(presentation);
	return createElement('div', {
		'data-projection-state': view.projectionState,
		ref: view.root,
	});
}

describe('useProjectedTerminalSurface', () => {
	it.each([
		{ kind: 'running' },
		{
			failure: { code: 'terminal-runtime-pty-read-read-output', retryable: false },
			kind: 'failed',
		},
	] as const)(
		'acquires projection for a $kind presentation and releases it on unmount',
		async (lifecycle) => {
			const release = vi.fn<() => void>();
			const open = vi.fn<TerminalProjectionClient['open']>(() => ({
				release,
				result: new Promise(() => undefined),
			}));
			const presentation: WorkbenchTerminalPresentation = {
				currentCwd: '/workspace',
				launchCwd: '/workspace',
				lifecycle,
				presentationId: terminalPresentationId('presentation-1'),
				title: 'Terminal 1',
			};
			const container = document.createElement('div');
			document.body.appendChild(container);
			const root = createRoot(container);

			await act(async () => {
				root.render(
					createElement(
						ProjectedTerminalSurfaceCapabilityContext.Provider,
						{
							value: {
								documentElement: document.documentElement,
								preferredLocales: ['en-US'],
								platformEffects: {},
								presentationConfigurationSource: DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
								projectionClient: { open },
								shortcutPlatform: 'apple',
								startStoppedTerminalSession: () => undefined,
							},
						},
						createElement(
							OrchbeeI18nProvider,
							{ locale: 'en-US' },
							createElement(TestHarness, { presentation }),
						),
					),
				);
			});

			expect(open).toHaveBeenCalledWith(presentation.presentationId);
			expect(container.firstElementChild?.getAttribute('data-projection-state')).toBe('requested');
			await act(async () => root.unmount());
			expect(release).toHaveBeenCalledOnce();
			container.remove();
		},
	);

	it('reopens a fresh projection after an attachment transport becomes unavailable', async () => {
		let resolveTransportUnavailable: (() => void) | undefined;
		const transportUnavailable = new Promise<void>((resolve) => {
			resolveTransportUnavailable = resolve;
		});
		const sessionDispose = vi.fn<() => void>();
		renderer.start.mockReset();
		renderer.start.mockResolvedValueOnce({
			dispose: sessionDispose,
			transportUnavailable,
			updatePresentationConfiguration: vi
				.fn<ProjectedTerminalRendererSession['updatePresentationConfiguration']>()
				.mockResolvedValue({
					configuration: DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE.current(),
					kind: 'completed',
				}),
		});
		const firstRelease = vi.fn<() => void>();
		const secondRelease = vi.fn<() => void>();
		const transport: ProjectedTerminalTransport = {
			close: vi.fn<() => void>(),
			projectionBufferTrust: 'external',
			send: vi.fn<ProjectedTerminalTransport['send']>(),
			subscribe: vi.fn<ProjectedTerminalTransport['subscribe']>(() => () => undefined),
		};
		let openSequence = 0;
		const open = vi.fn<TerminalProjectionClient['open']>(() => {
			openSequence += 1;
			if (openSequence === 1) {
				return {
					release: firstRelease,
					result: Promise.resolve({
						bootstrap: {
							attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
							canInput: true,
							canResize: true,
							creationId: CreationIdSchema.parse('22'.repeat(16)),
							hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
							terminalId: TerminalIdSchema.parse('44'.repeat(16)),
							terminalOutcome: null,
						},
						transport,
					}),
				};
			}
			return { release: secondRelease, result: new Promise(() => undefined) };
		});
		const presentation: WorkbenchTerminalPresentation = {
			currentCwd: '/workspace',
			launchCwd: '/workspace',
			lifecycle: { kind: 'running' },
			presentationId: terminalPresentationId('presentation-1'),
			title: 'Terminal 1',
		};
		const container = document.createElement('div');
		document.body.appendChild(container);
		const root = createRoot(container);

		await act(async () => {
			root.render(
				createElement(
					ProjectedTerminalSurfaceCapabilityContext.Provider,
					{
						value: {
							documentElement: document.documentElement,
							platformEffects: {},
							presentationConfigurationSource: DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
							preferredLocales: ['en-US'],
							projectionClient: { open },
							shortcutPlatform: 'apple',
							startStoppedTerminalSession: () => undefined,
						},
					},
					createElement(
						OrchbeeI18nProvider,
						{ locale: 'en-US' },
						createElement(TestHarness, { presentation }),
					),
				),
			);
			await Promise.resolve();
		});
		expect(open).toHaveBeenCalledOnce();
		expect(renderer.start).toHaveBeenCalledOnce();

		await act(async () => {
			resolveTransportUnavailable?.();
			await Promise.resolve();
		});
		expect(open).toHaveBeenCalledTimes(2);
		expect(firstRelease).toHaveBeenCalledOnce();
		expect(sessionDispose).toHaveBeenCalledOnce();

		await act(async () => root.unmount());
		expect(secondRelease).toHaveBeenCalledOnce();
		container.remove();
	});
});
