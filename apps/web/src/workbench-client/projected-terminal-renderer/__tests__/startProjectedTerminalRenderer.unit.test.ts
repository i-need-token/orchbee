import {
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
	CreationIdSchema,
	HostEpochSchema,
	TerminalIdSchema,
} from '@orchbee/terminal-protocol';
import type {
	ProjectedTerminal,
	ProjectedTerminalFrameTelemetry,
	ProjectedTerminalMetadata,
	ProjectedTerminalOptions,
} from '@orchbee/terminal';
import { DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION } from '@orchbee/terminal';
import { describe, expect, it, vi } from 'vitest';

import type { ProjectedTerminalTransport } from '../../terminal-projection';
import type { ProjectedTerminalPlatformEffects } from '../../projected-terminal-platform-effects';
import {
	startProjectedTerminalRenderer,
	type ProjectedTerminalRendererDependencies,
} from '../startProjectedTerminalRenderer';
import type { TerminalPresentationConfigurationSource } from '../terminal-presentation-configuration-source';

describe('startProjectedTerminalRenderer', () => {
	it('localizes the projected surface and commits frame events from only its scoped port', async () => {
		const dataset: DOMStringMap = {};
		const status = {
			dataset: {},
			hidden: false,
			remove: vi.fn<ChildNode['remove']>(),
			setAttribute: vi.fn<Element['setAttribute']>(),
			textContent: null,
		};
		const findRoot = {
			contains: vi.fn<HTMLElement['contains']>(() => false),
			dataset: {},
			remove: vi.fn<ChildNode['remove']>(),
		};
		const root = {
			appendChild: vi.fn<Node['appendChild']>(),
			contains: vi.fn<HTMLElement['contains']>(() => true),
			dataset,
			ownerDocument: {
				createElement: vi
					.fn<(tagName: string) => typeof status | typeof findRoot>()
					.mockReturnValueOnce(status)
					.mockReturnValueOnce(findRoot),
			},
			style: {},
		} as unknown as HTMLElement;
		const documentElement = { dir: '', lang: '' };
		const applyFrame = vi.fn<ProjectedTerminal['applyFrame']>(() => 'committed');
		const applyRows = vi.fn<ProjectedTerminal['applyRows']>(() => 'committed');
		const notifyLinkOpened = vi.fn<ProjectedTerminal['notifyLinkOpened']>();
		let emitBell: Parameters<ProjectedTerminal['onBell']>[0] = () => undefined;
		const notifyBell = vi.fn<ProjectedTerminal['notifyBell']>((event) => emitBell(event));
		const notifyExited = vi.fn<ProjectedTerminal['notifyExited']>();
		const notifyBrokerFailure = vi.fn<ProjectedTerminal['notifyBrokerFailure']>();
		const notifySelectionCopied = vi.fn<ProjectedTerminal['notifySelectionCopied']>();
		const disposeTerminal = vi.fn<ProjectedTerminal['dispose']>();
		const focusTerminal = vi.fn<ProjectedTerminal['focus']>();
		const openTerminal = vi.fn<ProjectedTerminal['open']>().mockResolvedValue(undefined);
		const updatePresentationConfiguration = vi
			.fn<ProjectedTerminal['updatePresentationConfiguration']>()
			.mockResolvedValue({
				configuration: DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
				kind: 'completed',
			});
		const writeClipboardText = vi.fn<
			NonNullable<ProjectedTerminalPlatformEffects['writeClipboardText']>
		>(async () => ({ kind: 'completed' as const }));
		const ringBell = vi.fn<NonNullable<ProjectedTerminalPlatformEffects['ringBell']>>();
		let emitExit: Parameters<ProjectedTerminal['onExit']>[0] = () => undefined;
		let emitFrameTelemetry: Parameters<ProjectedTerminal['onFrameTelemetry']>[0] = () => undefined;
		let emitLinkOpenRequest: Parameters<ProjectedTerminal['onLinkOpenRequest']>[0] = () =>
			undefined;
		let emitMetadata: Parameters<ProjectedTerminal['onMetadataChange']>[0] | undefined;
		const openExternal = vi
			.fn<NonNullable<ProjectedTerminalPlatformEffects['openExternal']>>()
			.mockResolvedValue({ kind: 'completed' });
		let options: ProjectedTerminalOptions | undefined;
		const dependencies: ProjectedTerminalRendererDependencies = {
			createTerminal: vi.fn<ProjectedTerminalRendererDependencies['createTerminal']>(
				(nextOptions) => {
					options = nextOptions;
					return {
						applyFrame,
						applyRows,
						canControlTerminal: true,
						clearSearch: vi.fn<ProjectedTerminal['clearSearch']>(),
						copySelectionAsync: vi
							.fn<ProjectedTerminal['copySelectionAsync']>()
							.mockResolvedValue({ kind: 'completed' }),
						dispose: disposeTerminal,
						findNext: vi.fn<ProjectedTerminal['findNext']>(() => false),
						findPrevious: vi.fn<ProjectedTerminal['findPrevious']>(() => false),
						focus: focusTerminal,
						getBufferRowsSnapshot: vi.fn<ProjectedTerminal['getBufferRowsSnapshot']>(() => ({
							generation: 1n,
							revision: 0n,
							totalRows: 0n,
						})),
						notifyExited,
						notifyBell,
						notifyBrokerFailure,
						notifyHistoryChanged: vi.fn<ProjectedTerminal['notifyHistoryChanged']>(),
						notifyLinkOpened,
						notifySelectionChanged: vi.fn<ProjectedTerminal['notifySelectionChanged']>(),
						notifySelectionCopied,
						onError: vi.fn<ProjectedTerminal['onError']>(() => ({
							dispose: vi.fn<() => void>(),
						})),
						onBell: vi.fn<ProjectedTerminal['onBell']>((listener) => {
							emitBell = listener;
							return { dispose: vi.fn<() => void>() };
						}),
						onExit: vi.fn<ProjectedTerminal['onExit']>((listener) => {
							emitExit = listener;
							return { dispose: vi.fn<() => void>() };
						}),
						onFrameTelemetry: vi.fn<ProjectedTerminal['onFrameTelemetry']>((listener) => {
							emitFrameTelemetry = listener;
							return { dispose: vi.fn<() => void>() };
						}),
						onHistoryChange: vi.fn<ProjectedTerminal['onHistoryChange']>(() => ({
							dispose: vi.fn<() => void>(),
						})),
						onLinkOpenRequest: vi.fn<ProjectedTerminal['onLinkOpenRequest']>((listener) => {
							emitLinkOpenRequest = listener;
							return { dispose: vi.fn<() => void>() };
						}),
						onMetadataChange: vi.fn<ProjectedTerminal['onMetadataChange']>((listener) => {
							emitMetadata = listener;
							return { dispose: vi.fn<() => void>() };
						}),
						onRendererHealth: vi.fn<ProjectedTerminal['onRendererHealth']>(() => ({
							dispose: vi.fn<() => void>(),
						})),
						onSearchResults: vi.fn<ProjectedTerminal['onSearchResults']>(() => ({
							dispose: vi.fn<() => void>(),
						})),
						onSelectionChange: vi.fn<ProjectedTerminal['onSelectionChange']>(() => ({
							dispose: vi.fn<() => void>(),
						})),
						onSelectionCopied: vi.fn<ProjectedTerminal['onSelectionCopied']>(() => ({
							dispose: vi.fn<() => void>(),
						})),
						openLinkAsync: vi
							.fn<ProjectedTerminal['openLinkAsync']>()
							.mockResolvedValue({ kind: 'completed' }),
						open: openTerminal,
						reset: vi.fn<ProjectedTerminal['reset']>(),
						selectAll: vi.fn<ProjectedTerminal['selectAll']>(),
						selection: { attachmentId: bootstrap.attachmentId, state: 'none' },
						updatePresentationConfiguration,
						updateSearch: vi.fn<ProjectedTerminal['updateSearch']>(),
					};
				},
			),
			installShortcuts: vi.fn<ProjectedTerminalRendererDependencies['installShortcuts']>(() =>
				vi.fn<() => void>(),
			),
			mountFindSurface: vi.fn<ProjectedTerminalRendererDependencies['mountFindSurface']>(() => ({
				dispose: vi.fn<() => void>(),
			})),
		};
		let emitTransportMessage: (value: unknown) => void = () => undefined;
		let emitTransportUnavailable: (() => void) | undefined;
		const transport: ProjectedTerminalTransport = {
			close: vi.fn<() => void>(),
			projectionBufferTrust: 'local-host',
			send: vi.fn<ProjectedTerminalTransport['send']>(),
			subscribe: vi.fn<ProjectedTerminalTransport['subscribe']>((listener, unavailable) => {
				emitTransportMessage = listener;
				emitTransportUnavailable = unavailable;
				return vi.fn<() => void>();
			}),
		};
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: false,
			canResize: false,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalOutcome: {
				event: {
					exit: { code: 0, kind: 'exit-code' as const },
					finalRevision: CanonicalUint64TextSchema.parse('1'),
					identity: {
						creationId: CreationIdSchema.parse('22'.repeat(16)),
						hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
						terminalId: TerminalIdSchema.parse('44'.repeat(16)),
					},
				},
				kind: 'exited' as const,
			},
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
		};
		const originalHTMLElement = globalThis.HTMLElement;
		let emitPresentationConfiguration: Parameters<
			TerminalPresentationConfigurationSource['subscribe']
		>[0] = () => undefined;
		const unsubscribePresentationConfiguration = vi.fn<() => void>();
		const initialPresentationConfiguration = {
			...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
			fontFamily: 'Injected Initial Mono',
		};
		const presentationConfigurationSource: TerminalPresentationConfigurationSource = {
			current: () => initialPresentationConfiguration,
			subscribe: (listener) => {
				emitPresentationConfiguration = listener;
				return unsubscribePresentationConfiguration;
			},
		};
		Object.defineProperty(globalThis, 'HTMLElement', {
			configurable: true,
			value: class {},
		});
		Object.setPrototypeOf(root, globalThis.HTMLElement.prototype);
		try {
			const session = await startProjectedTerminalRenderer(
				root,
				documentElement,
				['zh-CN'],
				'apple',
				bootstrap,
				transport,
				dependencies,
				{ openExternal, ringBell, writeClipboardText },
				presentationConfigurationSource,
			);
			expect(dependencies.createTerminal).toHaveBeenCalledWith(
				expect.objectContaining({ projectionBufferTrust: 'local-host' }),
			);
			emitTransportMessage({ frame: new Uint8Array([1, 2, 3]).buffer, kind: 'frame' });
			emitTransportMessage({
				event: { attachmentId: bootstrap.attachmentId, sequence: '5' },
				kind: 'bell',
			});
			emitTransportMessage({ kind: 'link-opened', requestSequence: '9' });
			emitTransportMessage({
				content: new TextEncoder().encode('copied from Host').buffer,
				kind: 'selection-copy-content',
				requestSequence: '10',
			});
			expect(applyFrame).toHaveBeenCalledOnce();
			expect(notifyBell).toHaveBeenCalledWith({
				attachmentId: bootstrap.attachmentId,
				sequence: '5',
			});
			expect(ringBell).toHaveBeenCalledWith({
				attachmentId: bootstrap.attachmentId,
				sequence: '5',
			});
			emitLinkOpenRequest({
				target: { kind: 'url', url: 'https://example.com/from-osc-8' },
			});
			await vi.waitFor(() =>
				expect(openExternal).toHaveBeenCalledWith('https://example.com/from-osc-8'),
			);
			expect(notifyLinkOpened).toHaveBeenCalledWith('9');
			await vi.waitFor(() => expect(writeClipboardText).toHaveBeenCalledWith('copied from Host'));
			expect(notifySelectionCopied).toHaveBeenCalledWith(16, '10');
			expect(notifyBrokerFailure).not.toHaveBeenCalled();
			const telemetry: ProjectedTerminalFrameTelemetry = {
				atlasGlyphCount: 0,
				atlasGlyphsCreated: 0,
				atlasHeight: 0,
				atlasPressure: 0,
				atlasUploadBytes: 0,
				atlasWidth: 0,
				backend: 'webgpu',
				backgroundInstances: 0,
				cellsVisited: 0,
				durationMs: 1,
				forceAll: false,
				frameId: 1,
				generation: CanonicalUint64TextSchema.parse('1'),
				glyphInstances: 0,
				gpuDrawCalls: 1,
				overlayInstances: 0,
				reasons: ['dirty'],
				revision: CanonicalUint64TextSchema.parse('1'),
				rowsRendered: 1,
				shapingRuns: 0,
				timestamp: 1,
				vertexUploadBytes: 0,
				viewportY: 0,
			};
			emitFrameTelemetry(telemetry);
			if (emitMetadata === undefined) throw new Error('metadata listener is missing');
			emitMetadata({
				cellPixelHeight: 18,
				cellPixelWidth: 9,
				columns: 80,
				defaultColors: {
					backgroundRgb: 0x12_34_56,
					cursorRgb: null,
					foregroundRgb: 0xfe_dc_ba,
				},
				rows: 24,
			} as ProjectedTerminalMetadata);
			emitExit({
				exit: { code: 0, kind: 'exit-code' },
				finalRevision: CanonicalUint64TextSchema.parse('1'),
				identity: {
					creationId: bootstrap.creationId,
					hostEpoch: bootstrap.hostEpoch,
					terminalId: bootstrap.terminalId,
				},
			});

			expect(documentElement).toEqual({ dir: 'ltr', lang: 'zh-CN' });
			expect(options?.labels).toEqual(
				expect.objectContaining({
					input: '终端输入',
					screen: '终端屏幕',
				}),
			);
			expect(options?.labels.deleted?.('甲')).toBe('已删除：甲');
			expect(options?.labels.row?.(2n, '中文')).toBe('终端第 2 行：中文');
			expect(options?.labels.screenCursor?.(3n, 4)).toBe('终端屏幕，光标位于第 3 行、第 4 列');
			expect(options?.labels.selectedText?.('内容')).toBe('已选择文本：内容');
			expect(options?.renderer).toEqual(initialPresentationConfiguration);
			expect(openTerminal).toHaveBeenCalledOnce();
			expect(focusTerminal).not.toHaveBeenCalled();
			expect(root.style.backgroundColor).toBe('#123456');
			expect(options?.inputEnabled).toBe(false);
			expect(notifyExited).toHaveBeenCalledWith(bootstrap.terminalOutcome.event);
			expect(dataset['projectedTerminalAttachmentId']).toBe(bootstrap.attachmentId);
			expect(dataset['projectedTerminalState']).toBe('exited');
			expect(dataset['projectedTerminalCommittedFrames']).toBe('1');
			expect(dataset['projectedTerminalFinalFramePresented']).toBe('true');
			expect(dataset['projectedTerminalRenderedGeneration']).toBe('1');
			expect(dataset['projectedTerminalRenderedRevision']).toBe('1');
			const runtimeConfiguration = {
				...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
				allowTransparency: true,
				fontFamily: 'Injected Runtime Mono',
			};
			emitPresentationConfiguration(runtimeConfiguration);
			await vi.waitFor(() =>
				expect(updatePresentationConfiguration).toHaveBeenCalledWith(runtimeConfiguration),
			);
			await vi.waitFor(() => {
				expect(dataset['projectedTerminalPresentationState']).toBe('completed');
				expect(dataset['projectedTerminalPresentationCommittedEpoch']).toBe('1');
			});
			emitFrameTelemetry({ ...telemetry, frameId: 2, timestamp: 2 });
			expect(dataset['projectedTerminalPresentationDrawnEpoch']).toBe('1');
			expect(dataset['projectedTerminalRenderedFrames']).toBe('2');
			expect(dependencies.createTerminal).toHaveBeenCalledOnce();
			expect(openTerminal).toHaveBeenCalledOnce();
			expect(options?.renderer).toEqual(initialPresentationConfiguration);
			if (emitTransportUnavailable === undefined) {
				throw new Error('transport unavailable listener is missing');
			}
			emitTransportUnavailable();
			expect(status).toMatchObject({
				dataset: { projectedTerminalStatus: 'true' },
				hidden: false,
				textContent: '终端连接失败。',
			});
			session.dispose();
			session.dispose();
			expect(unsubscribePresentationConfiguration).toHaveBeenCalledOnce();
			expect(disposeTerminal).toHaveBeenCalledOnce();

			const createAdditionalRoot = () => {
				const additionalStatus = {
					dataset: {},
					hidden: false,
					remove: vi.fn<ChildNode['remove']>(),
					setAttribute: vi.fn<Element['setAttribute']>(),
					textContent: null,
				};
				const additionalFindRoot = {
					contains: vi.fn<HTMLElement['contains']>(() => false),
					dataset: {},
					remove: vi.fn<ChildNode['remove']>(),
				};
				const additionalRoot = {
					appendChild: vi.fn<Node['appendChild']>(),
					contains: vi.fn<HTMLElement['contains']>(() => true),
					dataset: {} as DOMStringMap,
					ownerDocument: {
						createElement: vi
							.fn<(tagName: string) => typeof additionalStatus | typeof additionalFindRoot>()
							.mockReturnValueOnce(additionalStatus)
							.mockReturnValueOnce(additionalFindRoot),
					},
					style: {},
				} as unknown as HTMLElement;
				Object.setPrototypeOf(additionalRoot, globalThis.HTMLElement.prototype);
				return { additionalFindRoot, additionalRoot, additionalStatus };
			};
			const createAdditionalTransport = () => {
				const close = vi.fn<() => void>();
				const additionalTransport: ProjectedTerminalTransport = {
					close,
					projectionBufferTrust: 'local-host',
					send: vi.fn<ProjectedTerminalTransport['send']>(),
					subscribe: vi.fn<ProjectedTerminalTransport['subscribe']>(() => vi.fn<() => void>()),
				};
				return { additionalTransport, close };
			};
			const liveBootstrap = {
				attachmentId: bootstrap.attachmentId,
				canInput: bootstrap.canInput,
				canResize: bootstrap.canResize,
				creationId: bootstrap.creationId,
				hostEpoch: bootstrap.hostEpoch,
				terminalOutcome: null,
				terminalId: bootstrap.terminalId,
			};

			vi.mocked(dependencies.createTerminal).mockClear();
			disposeTerminal.mockClear();
			openTerminal.mockClear();
			const liveElements = createAdditionalRoot();
			const liveTransport = createAdditionalTransport();
			let emitLivePresentationConfiguration: Parameters<
				TerminalPresentationConfigurationSource['subscribe']
			>[0] = () => undefined;
			const unsubscribeLiveSource = vi.fn<() => void>();
			const liveSource: TerminalPresentationConfigurationSource = {
				current: () => initialPresentationConfiguration,
				subscribe: (listener) => {
					emitLivePresentationConfiguration = listener;
					return unsubscribeLiveSource;
				},
			};
			const liveSession = await startProjectedTerminalRenderer(
				liveElements.additionalRoot,
				documentElement,
				['en-US'],
				'apple',
				liveBootstrap,
				liveTransport.additionalTransport,
				dependencies,
				{},
				liveSource,
			);
			expect(liveElements.additionalRoot.dataset['projectedTerminalState']).toBe('open');
			updatePresentationConfiguration.mockRejectedValueOnce(new Error('injected update rejection'));
			emitLivePresentationConfiguration({
				...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
				fontFamily: 'Rejected Runtime Mono',
			});
			await vi.waitFor(() => {
				expect(liveElements.additionalRoot.dataset['projectedTerminalPresentationState']).toBe(
					'failed',
				);
				expect(liveElements.additionalRoot.dataset['projectedTerminalPresentationFailure']).toBe(
					'update-rejected',
				);
			});
			expect(liveElements.additionalRoot.dataset['projectedTerminalState']).toBe('open');
			expect(liveElements.additionalStatus.hidden).toBe(true);
			expect(liveTransport.close).not.toHaveBeenCalled();

			updatePresentationConfiguration.mockResolvedValueOnce({
				code: 'renderer-preparation-failed',
				kind: 'failed',
			});
			emitLivePresentationConfiguration({
				...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
				fontFamily: 'Renderer Failed Runtime Mono',
			});
			await vi.waitFor(() => {
				expect(liveElements.additionalRoot.dataset['projectedTerminalPresentationFailure']).toBe(
					'renderer-preparation-failed',
				);
			});
			expect(liveElements.additionalRoot.dataset['projectedTerminalState']).toBe('open');
			expect(liveElements.additionalStatus).toMatchObject({ hidden: true, textContent: null });
			expect(liveTransport.close).not.toHaveBeenCalled();
			liveSession.dispose();
			expect(unsubscribeLiveSource).toHaveBeenCalledOnce();
			expect(liveTransport.close).toHaveBeenCalledOnce();

			vi.mocked(dependencies.createTerminal).mockClear();
			disposeTerminal.mockClear();
			const currentFailureElements = createAdditionalRoot();
			const currentFailureTransport = createAdditionalTransport();
			await expect(
				startProjectedTerminalRenderer(
					currentFailureElements.additionalRoot,
					documentElement,
					['en-US'],
					'apple',
					liveBootstrap,
					currentFailureTransport.additionalTransport,
					dependencies,
					{},
					{
						current: () => {
							throw new Error('configuration current failed');
						},
						subscribe: () => vi.fn<() => void>(),
					},
				),
			).rejects.toThrow('configuration current failed');
			expect(currentFailureElements.additionalRoot.appendChild).not.toHaveBeenCalled();
			expect(dependencies.createTerminal).not.toHaveBeenCalled();
			expect(disposeTerminal).not.toHaveBeenCalled();
			expect(currentFailureTransport.close).toHaveBeenCalledOnce();

			vi.mocked(dependencies.createTerminal).mockClear();
			disposeTerminal.mockClear();
			const subscribeFailureElements = createAdditionalRoot();
			const subscribeFailureTransport = createAdditionalTransport();
			await expect(
				startProjectedTerminalRenderer(
					subscribeFailureElements.additionalRoot,
					documentElement,
					['en-US'],
					'apple',
					liveBootstrap,
					subscribeFailureTransport.additionalTransport,
					dependencies,
					{},
					{
						current: () => initialPresentationConfiguration,
						subscribe: () => {
							throw new Error('configuration subscribe failed');
						},
					},
				),
			).rejects.toThrow('configuration subscribe failed');
			expect(subscribeFailureElements.additionalStatus.remove).toHaveBeenCalledOnce();
			expect(dependencies.createTerminal).toHaveBeenCalledOnce();
			expect(disposeTerminal).toHaveBeenCalledOnce();
			expect(subscribeFailureTransport.close).toHaveBeenCalledOnce();

			vi.mocked(dependencies.createTerminal).mockClear();
			disposeTerminal.mockClear();
			const laterSetupFailureElements = createAdditionalRoot();
			const laterSetupFailureTransport = createAdditionalTransport();
			laterSetupFailureTransport.additionalTransport.subscribe = vi
				.fn<ProjectedTerminalTransport['subscribe']>()
				.mockImplementation(() => {
					throw new Error('transport subscribe failed');
				});
			const unsubscribeLaterSource = vi.fn<() => void>();
			await expect(
				startProjectedTerminalRenderer(
					laterSetupFailureElements.additionalRoot,
					documentElement,
					['en-US'],
					'apple',
					liveBootstrap,
					laterSetupFailureTransport.additionalTransport,
					dependencies,
					{},
					{
						current: () => initialPresentationConfiguration,
						subscribe: () => unsubscribeLaterSource,
					},
				),
			).rejects.toThrow('transport subscribe failed');
			expect(unsubscribeLaterSource).toHaveBeenCalledOnce();
			expect(laterSetupFailureElements.additionalStatus.remove).toHaveBeenCalledOnce();
			expect(disposeTerminal).toHaveBeenCalledOnce();
			expect(laterSetupFailureTransport.close).toHaveBeenCalledOnce();
		} finally {
			Object.defineProperty(globalThis, 'HTMLElement', {
				configurable: true,
				value: originalHTMLElement,
			});
		}
	});
});
