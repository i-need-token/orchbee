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
import {
	terminalPresentationId,
	terminalPresentationOperationId,
	terminalPresentationRemovalId,
	type WorkbenchTerminalPresentation,
	type WorkbenchTerminalSurfaceProps,
} from '@orchbee/workbench';
import { act, createElement, type ReactNode } from 'react';
import { createRoot } from 'react-dom/client';
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

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
	presentation,
}: {
	readonly autoFocus?: boolean | undefined;
	readonly focusRequest?: number | undefined;
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
				},
	);
	return createElement(
		'div',
		{
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
	it('focuses a requested terminal only after its painted attachment is ready', async () => {
		let markPresentationReady: (() => void) | undefined;
		const presentationReady = new Promise<void>((resolve) => {
			markPresentationReady = resolve;
		});
		const focus = vi.fn<() => void>();
		renderer.start.mockResolvedValueOnce({
			dispose: vi.fn<() => void>(),
			focus,
			freeze: vi.fn<() => void>(),
			isTransportAvailable: () => true,
			pasteFileDrop: vi.fn<ProjectedTerminalRendererSession['pasteFileDrop']>(),
			presentationReady,
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
			launchCwd: '/workspace',
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
			startStoppedTerminalSession: () => ({ kind: 'unchanged' as const }),
		};
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
							createElement(TestHarness, { autoFocus, focusRequest, presentation }),
						),
					),
				);
			});
		};

		await renderHarness(true, 1);
		await vi.waitFor(() => expect(renderer.start).toHaveBeenCalledOnce());
		expect(focus).not.toHaveBeenCalled();
		await act(async () => markPresentationReady?.());
		await vi.waitFor(() => expect(focus).toHaveBeenCalledOnce());

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
		{ kind: 'stopped' },
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

			expect(open).toHaveBeenCalledWith(presentation.presentationId, mountedGeometry);
			expect(container.firstElementChild?.getAttribute('data-projection-state')).toBe('requested');
			await act(async () => root.unmount());
			expect(release).toHaveBeenCalledOnce();
			container.remove();
		},
	);

	it('freezes the painted frame while a managed terminal closes', async () => {
		const mountedFrame = document.createElement('canvas');
		const sessionDispose = vi.fn<() => void>(() => mountedFrame.remove());
		const sessionFreeze = vi.fn<() => void>();
		renderer.start.mockImplementationOnce(async (rootElement) => {
			rootElement?.append(mountedFrame);
			return {
				dispose: sessionDispose,
				focus: vi.fn<() => void>(),
				freeze: sessionFreeze,
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
			};
		});
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
		const runningPresentation: WorkbenchTerminalPresentation = {
			closeActivity: 'idle',
			currentCwd: '/workspace',
			launchCwd: '/workspace',
			lifecycle: { kind: 'running' },
			naturalTitle: null,
			presentationId: terminalPresentationId('presentation-closing-frame'),
			title: 'Terminal Closing Frame',
		};
		const capability: ProjectedTerminalSurfaceCapability = {
			documentElement: document.documentElement,
			observeTerminalMetadata: () => undefined,
			platformEffects: {},
			preferredLocales: ['en-US'],
			presentationConfigurationSource: DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION_SOURCE,
			projectionClient: { open },
			shortcutPlatform: 'apple',
			startStoppedTerminalSession: () => ({ kind: 'unchanged' as const }),
		};
		const container = document.createElement('div');
		document.body.appendChild(container);
		const root = createRoot(container);
		const renderPresentation = async (
			presentation: WorkbenchTerminalPresentation,
		): Promise<void> => {
			await act(async () => {
				root.render(
					createElement(
						ProjectedTerminalSurfaceCapabilityContext.Provider,
						{ value: capability },
						createElement(
							OrchbeeI18nProvider,
							{ locale: 'en-US' },
							createElement(TestHarness, { presentation }),
						),
					),
				);
			});
		};

		await renderPresentation(runningPresentation);
		await vi.waitFor(() => expect(renderer.start).toHaveBeenCalledOnce());
		expect(container.contains(mountedFrame)).toBe(true);

		await renderPresentation({
			...runningPresentation,
			lifecycle: {
				kind: 'closing',
				operationId: terminalPresentationOperationId('close-painted-frame'),
				removalId: terminalPresentationRemovalId('remove-painted-frame'),
			},
		});

		expect(sessionFreeze).toHaveBeenCalledOnce();
		expect(release).toHaveBeenCalledOnce();
		expect(sessionDispose).not.toHaveBeenCalled();
		expect(container.contains(mountedFrame)).toBe(true);
		expect(open).toHaveBeenCalledOnce();

		await act(async () => root.unmount());
		expect(sessionDispose).toHaveBeenCalledOnce();
		container.remove();
	});

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
			launchCwd: '/workspace',
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
			focus: vi.fn<() => void>(),
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

	it('settles renderer paste reviews through explicit surface decisions', async () => {
		renderer.start.mockReset();
		renderer.start.mockResolvedValue({
			dispose: vi.fn<() => void>(),
			focus: vi.fn<() => void>(),
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

	it('blocks native file navigation and reports unavailable clients without a path capability', async () => {
		const presentation: WorkbenchTerminalPresentation = {
			closeActivity: 'unknown',
			currentCwd: '/workspace',
			launchCwd: '/workspace',
			lifecycle: { kind: 'stopped' },
			naturalTitle: null,
			presentationId: terminalPresentationId('presentation-file-drop-unavailable'),
			title: 'Terminal File Drop Unavailable',
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
									throw new Error('An unavailable file drop must not open a projection.');
								},
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
		const surface = container.firstElementChild;
		if (!(surface instanceof HTMLElement)) throw new Error('Terminal surface did not mount.');
		const selectedFile = new File([], 'unavailable.txt');
		const dragOver = terminalFileDragEvent('dragover', [], { types: ['Files'] });
		surface.dispatchEvent(dragOver.event);
		expect(dragOver.event.defaultPrevented).toBe(true);
		expect(dragOver.dataTransfer.dropEffect).toBe('none');
		const drop = terminalFileDragEvent('drop', [selectedFile]);
		await act(async () => {
			surface.dispatchEvent(drop.event);
		});
		expect(drop.event.defaultPrevented).toBe(true);
		expect(surface.getAttribute('data-failure-message')).toBe(
			'File drop is unavailable in this client.',
		);
		expect(renderer.start).not.toHaveBeenCalled();
		await act(async () => root.unmount());
		container.remove();
	});

	it('owns Desktop file drops, publishes only prepared text, and reports bounded rejection', async () => {
		const preparedText = '/tmp/space\\ name.txt';
		const preparedDirectoryText = '/Users/example/github/ghostty';
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
			focus: vi.fn<() => void>(),
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
		const prepare = vi
			.fn<
				NonNullable<ProjectedTerminalSurfaceCapability['terminalFileDropCapability']>['prepare']
			>()
			.mockResolvedValueOnce({
				kind: 'ready',
				paths: ['/tmp/space name.txt'],
				text: preparedText,
			})
			.mockResolvedValueOnce({
				kind: 'ready',
				paths: ['/Users/example/github/ghostty'],
				text: preparedDirectoryText,
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
		const protectedItem = vi.fn<() => File | null>(() => null);
		const dragOver = terminalFileDragEvent('dragover', [], {
			items: [{ getAsFile: protectedItem, kind: 'file' }],
			types: ['Files'],
		});
		surface.dispatchEvent(dragOver.event);
		expect(dragOver.event.defaultPrevented).toBe(true);
		expect(dragOver.dataTransfer.dropEffect).toBe('copy');
		expect(protectedItem).not.toHaveBeenCalled();

		const drop = terminalFileDragEvent('drop', [selectedFile]);
		await act(async () => {
			surface.dispatchEvent(drop.event);
			await Promise.resolve();
		});
		await vi.waitFor(() => expect(pasteFileDrop).toHaveBeenCalledExactlyOnceWith(preparedText));
		expect(drop.event.defaultPrevented).toBe(true);
		expect(prepare).toHaveBeenNthCalledWith(1, [selectedFile]);

		const selectedDirectory = new File([], 'ghostty');
		const directoryDrop = terminalFileDragEvent('drop', [], {
			items: [{ getAsFile: () => selectedDirectory, kind: 'file' }],
			types: ['Files'],
		});
		await act(async () => {
			surface.dispatchEvent(directoryDrop.event);
			await Promise.resolve();
		});
		await vi.waitFor(() => expect(pasteFileDrop).toHaveBeenNthCalledWith(2, preparedDirectoryText));
		expect(directoryDrop.event.defaultPrevented).toBe(true);
		expect(prepare).toHaveBeenNthCalledWith(2, [selectedDirectory]);

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
		expect(pasteFileDrop).toHaveBeenCalledTimes(2);

		let releaseStalePreparation: (() => void) | undefined;
		prepare.mockImplementationOnce(async () => {
			await new Promise<void>((resolve) => {
				releaseStalePreparation = resolve;
			});
			return {
				kind: 'ready',
				paths: ['/tmp/stale.txt'],
				text: '/tmp/stale.txt',
			};
		});
		await act(async () => {
			surface.dispatchEvent(terminalFileDragEvent('drop', [new File([], 'stale.txt')]).event);
			await Promise.resolve();
		});
		await vi.waitFor(() => expect(prepare).toHaveBeenCalledTimes(4));
		await act(async () => root.unmount());
		releaseStalePreparation?.();
		await Promise.resolve();
		await Promise.resolve();
		expect(pasteFileDrop).toHaveBeenCalledTimes(2);
		expect(release).toHaveBeenCalledOnce();
		container.remove();
	});
});

function terminalFileDragEvent(
	type: 'dragover' | 'drop',
	files: readonly File[],
	options?: {
		readonly items?: readonly { getAsFile(): File | null; readonly kind: string }[];
		readonly types?: readonly string[];
	},
): {
	readonly dataTransfer: {
		dropEffect: string;
		readonly files: readonly File[];
		readonly items: readonly { getAsFile(): File | null; readonly kind: string }[];
		readonly types: readonly string[];
	};
	readonly event: Event;
} {
	const dataTransfer = {
		dropEffect: 'none',
		files,
		items:
			options?.items ?? files.map((file) => ({ getAsFile: () => file, kind: 'file' as const })),
		types: options?.types ?? (files.length === 0 ? [] : ['Files']),
	};
	const event = new Event(type, { bubbles: true, cancelable: true });
	Object.defineProperty(event, 'dataTransfer', { value: dataTransfer });
	return { dataTransfer, event };
}
