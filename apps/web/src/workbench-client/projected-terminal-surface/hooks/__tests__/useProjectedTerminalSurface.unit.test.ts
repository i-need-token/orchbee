// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import {
	AttachmentIdSchema,
	CreationIdSchema,
	HostEpochSchema,
	TerminalIdSchema,
} from '@orchbee/terminal-protocol';
import type { ProjectedPasteReview, ProjectedTerminalMetadata } from '@orchbee/terminal';
import { terminalPresentationId, type WorkbenchTerminalPresentation } from '@orchbee/workbench';
import { act, createElement, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, afterEach, beforeAll, describe, expect, it, vi } from 'vitest';

import type {
	ProjectedTerminalTransport,
	TerminalProjectionClient,
} from '../../../terminal-projection';
import {
	DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
	type ProjectedTerminalRendererSession,
	type startProjectedTerminalRenderer,
} from '../../../projected-terminal-renderer';
import {
	ProjectedTerminalSurfaceCapabilityContext,
	type ProjectedTerminalSurfaceCapability,
} from '../../contexts/projected-terminal-surface-capability';
import { ProjectedTerminalRecoveryController } from '../../model/ProjectedTerminalRecoveryController';
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

afterEach(() => {
	vi.useRealTimers();
	renderer.start.mockReset();
});

function TestHarness({
	presentation,
}: {
	readonly presentation: WorkbenchTerminalPresentation;
}): ReactNode {
	const view = useProjectedTerminalSurface(presentation);
	return createElement(
		'div',
		{
			'data-failure-message': view.failureMessage,
			'data-paste-review-text': view.pasteReview?.originalText,
			'data-projection-state': view.projectionState,
			'data-recovery-available': view.recoveryAvailable,
			ref: view.root,
		},
		view.stopped
			? createElement(
					'button',
					{ 'data-start-new-session': true, onClick: view.startNewSession },
					'Start new session',
				)
			: null,
		view.pasteReview === null
			? null
			: createElement(
					'button',
					{ 'data-approve-paste': true, onClick: view.approvePaste },
					'Approve paste',
				),
		view.recoveryAvailable
			? createElement(
					'button',
					{ 'data-retry-projection': true, onClick: view.retryProjection },
					'Retry projection',
				)
			: null,
	);
}

describe('useProjectedTerminalSurface', () => {
	it('presents a rejected Start New Session result instead of dropping it', async () => {
		const startStoppedTerminalSession = vi
			.fn<ProjectedTerminalSurfaceCapability['startStoppedTerminalSession']>()
			.mockReturnValue({ kind: 'rejected', reason: 'terminal-creation-sealed' });
		const presentation: WorkbenchTerminalPresentation = {
			currentCwd: '/workspace',
			launchCwd: '/workspace',
			lifecycle: { kind: 'stopped' },
			naturalTitle: null,
			presentationId: terminalPresentationId('presentation-stopped'),
			title: 'Terminal Stopped',
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
							observeTerminalMetadata: () => undefined,
							platformEffects: {},
							preferredLocales: ['en-US'],
							presentationConfigurationSource: DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
							projectionClient: {
								open: () => {
									throw new Error('A stopped presentation must not open a projection.');
								},
							},
							shortcutPlatform: 'apple',
							startStoppedTerminalSession,
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
		expect(container.firstElementChild?.getAttribute('data-failure-message')).toBeNull();

		await act(async () => {
			(container.querySelector('[data-start-new-session]') as HTMLButtonElement).click();
		});

		expect(startStoppedTerminalSession).toHaveBeenCalledWith(presentation.presentationId);
		expect(container.firstElementChild?.getAttribute('data-failure-message')).toBe(
			'The terminal session could not be started.',
		);
		await act(async () => root.unmount());
		container.remove();
	});

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
				naturalTitle: null,
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
								observeTerminalMetadata: () => undefined,
								platformEffects: {},
								presentationConfigurationSource: DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
								projectionClient: { open },
								shortcutPlatform: 'apple',
								startStoppedTerminalSession: () => ({ kind: 'unchanged' as const }),
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

	it('disposes a candidate renderer that unmounts before its first presented frame', async () => {
		const sessionDispose = vi.fn<() => void>();
		renderer.start.mockResolvedValueOnce({
			dispose: sessionDispose,
			freeze: vi.fn<() => void>(),
			presentationReady: new Promise<void>(() => undefined),
			transportUnavailable: new Promise<void>(() => undefined),
			updatePresentationConfiguration: vi
				.fn<ProjectedTerminalRendererSession['updatePresentationConfiguration']>()
				.mockResolvedValue({
					configuration: DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE.current(),
					kind: 'completed',
				}),
		});
		const release = vi.fn<() => void>();
		const transport: ProjectedTerminalTransport = {
			close: vi.fn<() => void>(),
			projectionBufferTrust: 'external',
			send: vi.fn<ProjectedTerminalTransport['send']>(),
			subscribe: vi.fn<ProjectedTerminalTransport['subscribe']>(() => () => undefined),
		};
		const presentation: WorkbenchTerminalPresentation = {
			currentCwd: '/workspace',
			launchCwd: '/workspace',
			lifecycle: { kind: 'running' },
			naturalTitle: null,
			presentationId: terminalPresentationId('presentation-candidate'),
			title: 'Terminal Candidate',
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
							observeTerminalMetadata: () => undefined,
							platformEffects: {},
							preferredLocales: ['en-US'],
							presentationConfigurationSource: DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
							projectionClient: {
								open: () => ({
									release,
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
								}),
							},
							shortcutPlatform: 'apple',
							startStoppedTerminalSession: () => ({ kind: 'unchanged' as const }),
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
		await vi.waitFor(() => expect(renderer.start).toHaveBeenCalledOnce());

		await act(async () => root.unmount());
		expect(sessionDispose).toHaveBeenCalledOnce();
		expect(release).toHaveBeenCalledOnce();
		container.remove();
	});

	it('releases a projection opened synchronously during recovery disposal', () => {
		const release = vi.fn<() => void>();
		let controller: ProjectedTerminalRecoveryController | undefined;
		controller = new ProjectedTerminalRecoveryController({
			onStateChange: () => undefined,
			open: () => {
				controller?.dispose();
				return { release, result: new Promise(() => undefined) };
			},
			startRenderer: () => {
				throw new Error('Disposed recovery must not start a renderer.');
			},
		});
		controller.start();
		expect(release).toHaveBeenCalledOnce();
	});

	it('keeps the last frame mounted and retries an unavailable transport with bounded backoff', async () => {
		vi.useFakeTimers();
		let resolveTransportUnavailable: (() => void) | undefined;
		const transportUnavailable = new Promise<void>((resolve) => {
			resolveTransportUnavailable = resolve;
		});
		const sessionDispose = vi.fn<() => void>();
		const sessionFreeze = vi.fn<() => void>();
		renderer.start.mockReset();
		renderer.start.mockResolvedValueOnce({
			dispose: sessionDispose,
			freeze: sessionFreeze,
			presentationReady: Promise.resolve(),
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
		const thirdRelease = vi.fn<() => void>();
		const observeTerminalMetadata =
			vi.fn<ProjectedTerminalSurfaceCapability['observeTerminalMetadata']>();
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
			if (openSequence === 2) {
				return { release: secondRelease, result: Promise.reject(new Error('still unavailable')) };
			}
			return { release: thirdRelease, result: new Promise(() => undefined) };
		});
		const presentation: WorkbenchTerminalPresentation = {
			currentCwd: '/workspace',
			launchCwd: '/workspace',
			lifecycle: { kind: 'running' },
			naturalTitle: null,
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
							observeTerminalMetadata,
							presentationConfigurationSource: DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
							preferredLocales: ['en-US'],
							projectionClient: { open },
							shortcutPlatform: 'apple',
							startStoppedTerminalSession: () => ({ kind: 'unchanged' as const }),
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
		const mountedFrame = container.firstElementChild?.appendChild(document.createElement('canvas'));
		const observeProjectedMetadata = renderer.start.mock.calls[0]?.[10];
		expect(observeProjectedMetadata).toBeTypeOf('function');
		observeProjectedMetadata?.({
			currentDirectory: 'file://orchbee-host/workspace',
			title: 'vim',
		} as ProjectedTerminalMetadata);
		expect(observeTerminalMetadata).toHaveBeenCalledWith(presentation.presentationId, {
			currentDirectory: 'file://orchbee-host/workspace',
			title: 'vim',
		});

		await act(async () => {
			resolveTransportUnavailable?.();
			await Promise.resolve();
		});
		expect(container.firstElementChild?.contains(mountedFrame ?? null)).toBe(true);
		expect(open).toHaveBeenCalledOnce();
		expect(firstRelease).toHaveBeenCalledOnce();
		expect(sessionFreeze).toHaveBeenCalledOnce();
		expect(sessionDispose).not.toHaveBeenCalled();

		await act(async () => {
			await vi.advanceTimersByTimeAsync(250);
		});
		expect(open).toHaveBeenCalledTimes(2);
		expect(container.firstElementChild?.contains(mountedFrame ?? null)).toBe(true);
		expect(firstRelease).toHaveBeenCalledOnce();
		expect(sessionDispose).not.toHaveBeenCalled();

		await act(async () => {
			await vi.advanceTimersByTimeAsync(500);
		});
		expect(open).toHaveBeenCalledTimes(3);
		expect(container.firstElementChild?.contains(mountedFrame ?? null)).toBe(true);

		await act(async () => root.unmount());
		expect(firstRelease).toHaveBeenCalledOnce();
		expect(secondRelease).toHaveBeenCalledOnce();
		expect(thirdRelease).toHaveBeenCalledOnce();
		expect(sessionDispose).toHaveBeenCalledOnce();
		container.remove();
	});

	it('stops after the retry budget and exposes an immediate manual recovery', async () => {
		vi.useFakeTimers();
		renderer.start.mockReset();
		const release = vi.fn<() => void>();
		const open = vi.fn<TerminalProjectionClient['open']>(() => ({
			release,
			result: Promise.reject(new Error('projection unavailable')),
		}));
		const presentation: WorkbenchTerminalPresentation = {
			currentCwd: '/workspace',
			launchCwd: '/workspace',
			lifecycle: { kind: 'running' },
			naturalTitle: null,
			presentationId: terminalPresentationId('presentation-recovery-budget'),
			title: 'Terminal Recovery',
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
							observeTerminalMetadata: () => undefined,
							platformEffects: {},
							preferredLocales: ['en-US'],
							presentationConfigurationSource: DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
							projectionClient: { open },
							shortcutPlatform: 'apple',
							startStoppedTerminalSession: () => ({ kind: 'unchanged' as const }),
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
		await act(async () => {
			await vi.waitFor(() => expect(vi.getTimerCount()).toBe(1));
			await vi.advanceTimersByTimeAsync(250);
		});
		await act(async () => {
			await vi.waitFor(() => expect(open).toHaveBeenCalledTimes(2));
			await vi.waitFor(() => expect(vi.getTimerCount()).toBe(1));
			await vi.advanceTimersByTimeAsync(500);
		});
		await act(async () => {
			await vi.waitFor(() => expect(open).toHaveBeenCalledTimes(3));
			await vi.waitFor(() => expect(vi.getTimerCount()).toBe(1));
			await vi.advanceTimersByTimeAsync(1_000);
		});

		expect(open).toHaveBeenCalledTimes(4);
		expect(container.firstElementChild?.getAttribute('data-recovery-available')).toBe('true');
		expect(container.firstElementChild?.getAttribute('data-failure-message')).toBe(
			'The terminal connection could not be restored.',
		);
		await act(async () => {
			(container.querySelector('[data-retry-projection]') as HTMLButtonElement).click();
			await Promise.resolve();
		});
		expect(open).toHaveBeenCalledTimes(5);

		await act(async () => root.unmount());
		expect(release).toHaveBeenCalledTimes(5);
		container.remove();
	});

	it('turns a renderer paste review into an explicit surface decision', async () => {
		renderer.start.mockReset();
		renderer.start.mockResolvedValue({
			dispose: vi.fn<() => void>(),
			freeze: vi.fn<() => void>(),
			presentationReady: Promise.resolve(),
			transportUnavailable: new Promise<void>(() => undefined),
			updatePresentationConfiguration: vi
				.fn<ProjectedTerminalRendererSession['updatePresentationConfiguration']>()
				.mockResolvedValue({
					configuration: DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE.current(),
					kind: 'completed',
				}),
		});
		const transport: ProjectedTerminalTransport = {
			close: vi.fn<() => void>(),
			projectionBufferTrust: 'external',
			send: vi.fn<ProjectedTerminalTransport['send']>(),
			subscribe: vi.fn<ProjectedTerminalTransport['subscribe']>(() => () => undefined),
		};
		const release = vi.fn<() => void>();
		const open = vi.fn<TerminalProjectionClient['open']>(() => ({
			release,
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
		}));
		const presentation: WorkbenchTerminalPresentation = {
			currentCwd: '/workspace',
			launchCwd: '/workspace',
			lifecycle: { kind: 'running' },
			naturalTitle: null,
			presentationId: terminalPresentationId('presentation-review'),
			title: 'Terminal Review',
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
							observeTerminalMetadata: () => undefined,
							presentationConfigurationSource: DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
							preferredLocales: ['en-US'],
							projectionClient: { open },
							shortcutPlatform: 'apple',
							startStoppedTerminalSession: () => ({ kind: 'unchanged' as const }),
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
		await vi.waitFor(() => expect(renderer.start).toHaveBeenCalledOnce());
		const reviewPaste = renderer.start.mock.calls[0]?.[9];
		expect(reviewPaste).toBeTypeOf('function');
		const review: ProjectedPasteReview = {
			bracketed: false,
			embeddedBracketTerminator: false,
			multiline: true,
			originalText: 'one\ntwo',
			replacedControlCharacters: 0,
			source: 'clipboard',
			text: 'one\ntwo',
		};
		let decision: Promise<boolean> | undefined;
		const reviewController = new AbortController();
		await act(async () => {
			decision = Promise.resolve(reviewPaste?.(review, reviewController.signal) ?? false);
			await Promise.resolve();
		});
		expect(container.firstElementChild?.getAttribute('data-paste-review-text')).toBe('one\ntwo');
		await act(async () => {
			(container.querySelector('[data-approve-paste]') as HTMLButtonElement).click();
			await Promise.resolve();
		});
		await expect(decision).resolves.toBe(true);
		expect(container.firstElementChild?.getAttribute('data-paste-review-text')).toBeNull();

		const cancelledController = new AbortController();
		let cancelledDecision: Promise<boolean> | undefined;
		await act(async () => {
			cancelledDecision = Promise.resolve(
				reviewPaste?.(review, cancelledController.signal) ?? false,
			);
			await Promise.resolve();
		});
		expect(container.firstElementChild?.getAttribute('data-paste-review-text')).toBe('one\ntwo');
		await act(async () => {
			cancelledController.abort();
			await Promise.resolve();
		});
		await expect(cancelledDecision).resolves.toBe(false);
		expect(container.firstElementChild?.getAttribute('data-paste-review-text')).toBeNull();

		await act(async () => root.unmount());
		expect(release).toHaveBeenCalledOnce();
		container.remove();
	});
});
