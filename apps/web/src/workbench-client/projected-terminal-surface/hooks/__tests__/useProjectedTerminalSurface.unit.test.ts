// @vitest-environment jsdom

import { ActionRuntime } from '@orchbee/actions';
import { OrchbeeI18nProvider } from '@orchbee/i18n';
import {
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
	CreationIdSchema,
	HostEpochSchema,
	TerminalIdSchema,
	TerminalThemeStateSchema,
} from '@orchbee/terminal-protocol';
import type {
	measureMountedTerminalGeometry,
	ProjectedPasteReview,
	ProjectedTerminalMetadata,
} from '@orchbee/terminal';
import { DEFAULT_TERMINAL_INTERACTION_CONFIGURATION_SOURCE } from '@orchbee/terminal';
import type {
	WorkbenchTerminalPresentation,
	WorkbenchTerminalSurfaceProps,
} from '../../../../workbench';
import { act, createElement, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';
import { terminalPresentationId } from '@orchbee/workbench-client-protocol';

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
import { useProjectedTerminalSurface } from '../useProjectedTerminalSurface';

const renderer = vi.hoisted(() => ({
	start: vi.fn<typeof startProjectedTerminalRenderer>(),
}));
const geometryMeasurement = vi.hoisted(() => ({
	measure: vi.fn<typeof measureMountedTerminalGeometry>(),
}));

const mountedGeometry = { cellPixelHeight: 18, cellPixelWidth: 9, columns: 96, rows: 32 } as const;
const workbenchActionRuntime = new ActionRuntime();
const workbenchActionScope = workbenchActionRuntime.createScope('test.projected-terminal-surface');
const admitBellEffect: WorkbenchTerminalSurfaceProps['admitBellEffect'] = () => true;
const bindActionContextProvider: WorkbenchTerminalSurfaceProps['bindActionContextProvider'] =
	() => ({
		dispose: () => undefined,
	});
const theme = TerminalThemeStateSchema.parse({
	operationId: null,
	revision: '0',
	terminal: {
		colorScheme: 'dark',
		defaultBackgroundRgb: 0x1e_1e_1e,
		defaultCursorRgb: null,
		defaultForegroundRgb: 0xd4_d4_d4,
		palette: Array.from({ length: 256 }, (_, index) => index),
	},
});

vi.mock(import('@orchbee/terminal'), async (importOriginal) => ({
	...(await importOriginal()),
	measureMountedTerminalGeometry: geometryMeasurement.measure,
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
	workbenchActionRuntime.dispose();
	Object.defineProperty(globalThis, 'IS_REACT_ACT_ENVIRONMENT', {
		configurable: true,
		value: false,
	});
});

beforeEach(() => {
	geometryMeasurement.measure.mockResolvedValue(mountedGeometry);
});

afterEach(() => {
	vi.useRealTimers();
	vi.unstubAllGlobals();
	geometryMeasurement.measure.mockReset();
	renderer.start.mockReset();
});

function TestHarness({
	autoFocus,
	focusRequest = 0,
	onPresentationReady = () => undefined,
	presentation,
}: {
	readonly autoFocus?: boolean | undefined;
	readonly focusRequest?: number | undefined;
	readonly onPresentationReady?: WorkbenchTerminalSurfaceProps['onPresentationReady'] | undefined;
	readonly presentation: WorkbenchTerminalPresentation;
}): ReactNode {
	const view = useProjectedTerminalSurface(
		presentation,
		autoFocus === undefined
			? undefined
			: {
					actionRuntime: workbenchActionRuntime,
					actionScope: workbenchActionScope,
					admitBellEffect,
					autoFocus,
					bindActionContextProvider,
					focusRequest,
					onPresentationReady,
				},
	);
	return createElement(
		'div',
		{
			'data-failure-message': view.failureMessage,
			'data-paste-review-text': view.pasteReview?.originalText,
			'data-projection-state': view.projectionState,
			'data-recovery-available': view.recoveryAvailable,
			ref: view.root,
		},
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

function createRunningTerminalFixture(presentationReady: Promise<void> = Promise.resolve()) {
	const focus = vi.fn<() => void>();
	const session = {
		dispose: vi.fn<() => void>(),
		focus,
		freeze: vi.fn<() => void>(),
		isTransportAvailable: () => true,
		presentationReady,
		transportUnavailable: new Promise<void>(() => undefined),
		updatePresentationConfiguration: vi
			.fn<ProjectedTerminalRendererSession['updatePresentationConfiguration']>()
			.mockResolvedValue({
				configuration: DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE.current(),
				kind: 'completed',
			}),
	} satisfies ProjectedTerminalRendererSession;
	renderer.start.mockResolvedValueOnce(session);
	const release = vi.fn<() => void>();
	const transport: ProjectedTerminalTransport = {
		close: vi.fn<() => void>(),
		projectionBufferTrust: 'external',
		send: vi.fn<ProjectedTerminalTransport['send']>(),
		subscribe: vi.fn<ProjectedTerminalTransport['subscribe']>(() => () => undefined),
	};
	const open = vi.fn<TerminalProjectionClient['open']>(() => ({
		release,
		result: Promise.resolve({
			bootstrap: {
				attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
				canInput: true,
				canResize: true,
				capabilityRevision: CanonicalUint64TextSchema.parse('1'),
				creationId: CreationIdSchema.parse('22'.repeat(16)),
				hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
				terminalId: TerminalIdSchema.parse('44'.repeat(16)),
				terminalOutcome: null,
				theme,
			},
			transport,
		}),
	}));
	const presentation: WorkbenchTerminalPresentation = {
		closeActivity: 'unknown',
		currentCwd: '/workspace',
		lifecycle: { kind: 'running' },
		naturalTitle: null,
		presentationId: terminalPresentationId('presentation-auto-focus'),
		title: 'Terminal Auto Focus',
	};
	const capability: ProjectedTerminalSurfaceCapability = {
		documentElement: document.documentElement,
		observeTerminalMetadata: () => undefined,
		platformEffects: {},
		preferredLocales: ['en-US'],
		interactionConfigurationSource: DEFAULT_TERMINAL_INTERACTION_CONFIGURATION_SOURCE,
		presentationConfigurationSource: DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
		projectionClient: { open },
		shortcutPlatform: 'apple',
	};
	return { capability, focus, presentation, release, session };
}

describe('useProjectedTerminalSurface', () => {
	it('reports initial display readiness and focuses only after the attachment paints', async () => {
		let markPresentationReady: (() => void) | undefined;
		const presentationReady = new Promise<void>((resolve) => {
			markPresentationReady = resolve;
		});
		const { capability, focus, presentation, release } =
			createRunningTerminalFixture(presentationReady);
		const onPresentationReady = vi.fn<WorkbenchTerminalSurfaceProps['onPresentationReady']>();
		const container = document.createElement('div');
		document.body.appendChild(container);
		const root = createRoot(container);
		const renderHarness = async (autoFocus: boolean, focusRequest: number): Promise<void> => {
			await act(async () => {
				root.render(
					createElement(
						ProjectedTerminalSurfaceCapabilityContext.Provider,
						{ value: capability },
						createElement(
							OrchbeeI18nProvider,
							{ locale: 'en-US' },
							createElement(TestHarness, {
								autoFocus,
								focusRequest,
								onPresentationReady,
								presentation,
							}),
						),
					),
				);
			});
		};

		await renderHarness(true, 1);
		await vi.waitFor(() => expect(renderer.start).toHaveBeenCalledOnce());
		expect(focus).not.toHaveBeenCalled();
		expect(onPresentationReady).not.toHaveBeenCalled();
		await act(async () => markPresentationReady?.());
		await vi.waitFor(() => expect(focus).toHaveBeenCalledOnce());
		expect(onPresentationReady).toHaveBeenCalledWith('display-ready');
		expect(onPresentationReady).toHaveBeenCalledWith('activation-ready');

		await renderHarness(true, 1);
		expect(focus).toHaveBeenCalledOnce();
		await renderHarness(true, 2);
		await vi.waitFor(() => expect(focus).toHaveBeenCalledTimes(2));
		await renderHarness(false, 3);
		expect(focus).toHaveBeenCalledTimes(2);
		await renderHarness(true, 3);
		await vi.waitFor(() => expect(focus).toHaveBeenCalledTimes(3));

		await act(async () => root.unmount());
		expect(release).toHaveBeenCalledOnce();
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

			expect(open).toHaveBeenCalledWith(presentation.presentationId, mountedGeometry);
			expect(container.firstElementChild?.getAttribute('data-projection-state')).toBe('requested');
			await act(async () => root.unmount());
			expect(release).toHaveBeenCalledOnce();
			container.remove();
		},
	);

	it('waits for positive mounted geometry before opening the projection', async () => {
		let notifyResize = (): void => undefined;
		class TestResizeObserver {
			constructor(callback: ResizeObserverCallback) {
				notifyResize = () => callback([], this as unknown as ResizeObserver);
			}

			disconnect(): void {}
			observe(): void {}
			unobserve(): void {}
		}
		vi.stubGlobal('ResizeObserver', TestResizeObserver);
		geometryMeasurement.measure
			.mockResolvedValueOnce(undefined)
			.mockResolvedValueOnce(mountedGeometry);
		const open = vi.fn<TerminalProjectionClient['open']>(() => ({
			release: vi.fn<() => void>(),
			result: new Promise(() => undefined),
		}));
		const presentation: WorkbenchTerminalPresentation = {
			closeActivity: 'unknown',
			currentCwd: '/workspace',
			lifecycle: { kind: 'running' },
			naturalTitle: null,
			presentationId: terminalPresentationId('presentation-awaiting-geometry'),
			title: 'Terminal Awaiting Geometry',
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
		expect(open).not.toHaveBeenCalled();

		await act(async () => notifyResize());
		await vi.waitFor(() =>
			expect(open).toHaveBeenCalledWith(presentation.presentationId, mountedGeometry),
		);
		await act(async () => root.unmount());
		container.remove();
	});

	it('disposes a candidate renderer that unmounts before its first presented frame', async () => {
		const sessionDispose = vi.fn<() => void>();
		renderer.start.mockResolvedValueOnce({
			dispose: sessionDispose,
			focus: vi.fn<() => void>(),
			freeze: vi.fn<() => void>(),
			isTransportAvailable: () => true,
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
											capabilityRevision: CanonicalUint64TextSchema.parse('1'),
											creationId: CreationIdSchema.parse('22'.repeat(16)),
											hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
											terminalId: TerminalIdSchema.parse('44'.repeat(16)),
											terminalOutcome: null,
											theme,
										},
										transport,
									}),
								}),
							},
							shortcutPlatform: 'apple',
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
			focus: vi.fn<() => void>(),
			freeze: sessionFreeze,
			isTransportAvailable: () => true,
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
							capabilityRevision: CanonicalUint64TextSchema.parse('1'),
							creationId: CreationIdSchema.parse('22'.repeat(16)),
							hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
							terminalId: TerminalIdSchema.parse('44'.repeat(16)),
							terminalOutcome: null,
							theme,
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
		expect(container.firstElementChild?.getAttribute('data-failure-message')).toBe('Disconnected');
		await act(async () => {
			(container.querySelector('[data-retry-projection]') as HTMLButtonElement).click();
			await Promise.resolve();
		});
		expect(open).toHaveBeenCalledTimes(5);

		await act(async () => root.unmount());
		expect(release).toHaveBeenCalledTimes(5);
		container.remove();
	});

	it('settles renderer paste reviews through explicit surface decisions', async () => {
		renderer.start.mockReset();
		renderer.start.mockResolvedValue({
			dispose: vi.fn<() => void>(),
			focus: vi.fn<() => void>(),
			freeze: vi.fn<() => void>(),
			isTransportAvailable: () => true,
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
					capabilityRevision: CanonicalUint64TextSchema.parse('1'),
					creationId: CreationIdSchema.parse('22'.repeat(16)),
					hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
					terminalId: TerminalIdSchema.parse('44'.repeat(16)),
					terminalOutcome: null,
					theme,
				},
				transport,
			}),
		}));
		const presentation: WorkbenchTerminalPresentation = {
			closeActivity: 'unknown',
			currentCwd: '/workspace',
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
