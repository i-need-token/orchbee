// @vitest-environment jsdom

import { OrchbeeI18nProvider } from '@orchbee/i18n';
import {
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
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
			'data-clipboard-write-text': view.clipboardWriteRequest?.text,
			'data-failure-message': view.failureMessage,
			'data-paste-review-text': view.pasteReview?.originalText,
			'data-projection-state': view.projectionState,
			'data-recovery-available': view.recoveryAvailable,
			onDragOver: view.handleFileDragOver,
			onDrop: view.handleFileDrop,
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
		view.clipboardWriteRequest === null
			? null
			: createElement(
					'button',
					{ 'data-approve-clipboard-write': true, onClick: view.approveClipboardWrite },
					'Approve clipboard write',
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
			closeActivity: 'unknown',
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
				closeActivity: 'unknown',
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
			isTransportAvailable: () => true,
			pasteFileDrop: vi.fn<ProjectedTerminalRendererSession['pasteFileDrop']>(),
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
			closeActivity: 'unknown',
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
			isTransportAvailable: () => true,
			pasteFileDrop: vi.fn<ProjectedTerminalRendererSession['pasteFileDrop']>(),
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
			closeActivity: 'unknown',
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
			closeActivity: 'unknown',
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

	it('serializes renderer paste and clipboard reviews into explicit surface decisions', async () => {
		renderer.start.mockReset();
		renderer.start.mockResolvedValue({
			dispose: vi.fn<() => void>(),
			freeze: vi.fn<() => void>(),
			isTransportAvailable: () => true,
			pasteFileDrop: vi.fn<ProjectedTerminalRendererSession['pasteFileDrop']>(),
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
			closeActivity: 'unknown',
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
		const reviewClipboardWrite = renderer.start.mock.calls[0]?.[12];
		expect(reviewPaste).toBeTypeOf('function');
		expect(reviewClipboardWrite).toBeTypeOf('function');
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

		const clipboardController = new AbortController();
		let clipboardDecision: Promise<boolean> | undefined;
		await act(async () => {
			clipboardDecision = Promise.resolve(
				reviewClipboardWrite?.(
					{
						attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
						sequence: CanonicalUint64TextSchema.parse('1'),
						text: 'exact clipboard text',
					},
					clipboardController.signal,
				) ?? false,
			);
			await Promise.resolve();
		});
		expect(container.firstElementChild?.getAttribute('data-clipboard-write-text')).toBe(
			'exact clipboard text',
		);
		await act(async () => {
			(container.querySelector('[data-approve-clipboard-write]') as HTMLButtonElement).click();
			await Promise.resolve();
		});
		await expect(clipboardDecision).resolves.toBe(true);
		expect(container.firstElementChild?.getAttribute('data-clipboard-write-text')).toBeNull();

		const cancelledController = new AbortController();
		let cancelledDecision: Promise<boolean> | undefined;
		await act(async () => {
			cancelledDecision = Promise.resolve(
				reviewPaste?.(review, cancelledController.signal) ?? false,
			);
			await Promise.resolve();
		});
		expect(container.firstElementChild?.getAttribute('data-paste-review-text')).toBe('one\ntwo');
		const rejectedClipboard = Promise.resolve(
			reviewClipboardWrite?.(
				{
					attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
					sequence: CanonicalUint64TextSchema.parse('2'),
					text: 'must not stack',
				},
				new AbortController().signal,
			) ?? false,
		);
		await expect(rejectedClipboard).resolves.toBe(false);
		expect(container.firstElementChild?.getAttribute('data-clipboard-write-text')).toBeNull();
		await act(async () => {
			cancelledController.abort();
			await Promise.resolve();
		});
		await expect(cancelledDecision).resolves.toBe(false);
		expect(container.firstElementChild?.getAttribute('data-paste-review-text')).toBeNull();

		let unmountedClipboardDecision: Promise<boolean> | undefined;
		await act(async () => {
			unmountedClipboardDecision = Promise.resolve(
				reviewClipboardWrite?.(
					{
						attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
						sequence: CanonicalUint64TextSchema.parse('3'),
						text: 'cancel on unmount',
					},
					new AbortController().signal,
				) ?? false,
			);
			await Promise.resolve();
		});
		expect(container.firstElementChild?.getAttribute('data-clipboard-write-text')).toBe(
			'cancel on unmount',
		);
		await act(async () => root.unmount());
		await expect(unmountedClipboardDecision).resolves.toBe(false);
		expect(release).toHaveBeenCalledOnce();
		container.remove();
	});

	it('owns Desktop file drops, publishes only prepared text, and reports bounded rejection', async () => {
		const preparedText = "'/tmp/space name.txt'";
		const pasteFileDrop = vi
			.fn<ProjectedTerminalRendererSession['pasteFileDrop']>()
			.mockResolvedValue({
				kind: 'sent',
				review: {
					bracketed: false,
					embeddedBracketTerminator: false,
					multiline: false,
					originalText: preparedText,
					replacedControlCharacters: 0,
					source: 'file-drop',
					text: preparedText,
				},
			});
		renderer.start.mockResolvedValue({
			dispose: vi.fn<() => void>(),
			freeze: vi.fn<() => void>(),
			isTransportAvailable: () => true,
			pasteFileDrop,
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
		const prepare = vi
			.fn<
				NonNullable<ProjectedTerminalSurfaceCapability['terminalFileDropCapability']>['prepare']
			>()
			.mockResolvedValueOnce({
				containsNewline: false,
				kind: 'ready',
				paths: ['/tmp/space name.txt'],
				text: preparedText,
			})
			.mockResolvedValueOnce({ kind: 'rejected', reason: 'too-many-files' });
		const presentation: WorkbenchTerminalPresentation = {
			closeActivity: 'unknown',
			currentCwd: '/workspace',
			launchCwd: '/workspace',
			lifecycle: { kind: 'running' },
			naturalTitle: null,
			presentationId: terminalPresentationId('presentation-file-drop'),
			title: 'Terminal File Drop',
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
							terminalFileDropCapability: { prepare },
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
			await Promise.resolve();
			await Promise.resolve();
		});
		await vi.waitFor(() => expect(renderer.start).toHaveBeenCalledOnce());
		const surface = container.firstElementChild;
		if (!(surface instanceof HTMLElement)) throw new Error('Terminal surface did not mount.');
		const selectedFile = new File([], 'space name.txt');
		const dragOver = terminalFileDragEvent('dragover', [selectedFile]);
		surface.dispatchEvent(dragOver.event);
		expect(dragOver.event.defaultPrevented).toBe(true);
		expect(dragOver.dataTransfer.dropEffect).toBe('copy');

		const drop = terminalFileDragEvent('drop', [selectedFile]);
		await act(async () => {
			surface.dispatchEvent(drop.event);
			await Promise.resolve();
		});
		await vi.waitFor(() => expect(pasteFileDrop).toHaveBeenCalledExactlyOnceWith(preparedText));
		expect(drop.event.defaultPrevented).toBe(true);
		expect(prepare).toHaveBeenNthCalledWith(1, [selectedFile]);

		const tooMany = Array.from({ length: 33 }, (_, index) => new File([], `file-${index}`));
		await act(async () => {
			surface.dispatchEvent(terminalFileDragEvent('drop', tooMany).event);
			await Promise.resolve();
		});
		await vi.waitFor(() =>
			expect(surface.getAttribute('data-failure-message')).toContain(
				'33 files were dropped. Select no more than 32.',
			),
		);
		expect(pasteFileDrop).toHaveBeenCalledOnce();

		let releaseStalePreparation: (() => void) | undefined;
		prepare.mockImplementationOnce(async () => {
			await new Promise<void>((resolve) => {
				releaseStalePreparation = resolve;
			});
			return {
				containsNewline: false,
				kind: 'ready',
				paths: ['/tmp/stale.txt'],
				text: "'/tmp/stale.txt'",
			};
		});
		await act(async () => {
			surface.dispatchEvent(terminalFileDragEvent('drop', [new File([], 'stale.txt')]).event);
			await Promise.resolve();
		});
		await vi.waitFor(() => expect(prepare).toHaveBeenCalledTimes(3));
		await act(async () => root.unmount());
		releaseStalePreparation?.();
		await Promise.resolve();
		await Promise.resolve();
		expect(pasteFileDrop).toHaveBeenCalledOnce();
		expect(release).toHaveBeenCalledOnce();
		container.remove();
	});
});

function terminalFileDragEvent(
	type: 'dragover' | 'drop',
	files: readonly File[],
): {
	readonly dataTransfer: { dropEffect: string; readonly files: readonly File[] };
	readonly event: Event;
} {
	const dataTransfer = { dropEffect: 'none', files };
	const event = new Event(type, { bubbles: true, cancelable: true });
	Object.defineProperty(event, 'dataTransfer', { value: dataTransfer });
	return { dataTransfer, event };
}
