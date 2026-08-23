import { ActionRuntime } from '@orchbee/actions';
import type {
	WorkbenchTerminalActionContextProvider,
	WorkbenchTerminalBellEffect,
} from '@orchbee/workbench';
import {
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
	CreationIdSchema,
	HostEpochSchema,
	TerminalIdSchema,
	TerminalThemeStateSchema,
} from '@orchbee/terminal-protocol';
import type {
	ProjectedTerminal,
	ProjectedTerminalErrorEvent,
	ProjectedTerminalFrameTelemetry,
	ProjectedTerminalMetadata,
	ProjectedTerminalOptions,
	TerminalInteractionConfigurationSource,
	TerminalPresentationConfiguration,
} from '@orchbee/terminal';
import { DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION } from '@orchbee/terminal';
import { describe, expect, it, vi } from 'vitest';

import type { ProjectedTerminalTransport } from '../../terminal-projection';
import type { ProjectedTerminalPlatformEffects } from '../../projected-terminal-platform-effects';
import type { TerminalCompletionReminderObserver } from '../../../terminal-feedback-settings';
import type { TerminalInputConfigurationSource } from '../../../terminal-input-configuration';
import {
	startProjectedTerminalRenderer,
	type ProjectedTerminalRendererDependencies,
} from '../startProjectedTerminalRenderer';
import type { TerminalPresentationConfigurationSource } from '../../../terminal-presentation';

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
		const progressIndicator = {
			setAttribute: vi.fn<Element['setAttribute']>(),
		};
		const progress = {
			appendChild: vi.fn<Node['appendChild']>(),
			dataset: {},
			hidden: false,
			remove: vi.fn<ChildNode['remove']>(),
			removeAttribute: vi.fn<Element['removeAttribute']>(),
			setAttribute: vi.fn<Element['setAttribute']>(),
			style: {
				removeProperty: vi.fn<CSSStyleDeclaration['removeProperty']>(),
				setProperty: vi.fn<CSSStyleDeclaration['setProperty']>(),
			},
		};
		const root = {
			appendChild: vi.fn<Node['appendChild']>(),
			contains: vi.fn<HTMLElement['contains']>(() => true),
			dataset,
			ownerDocument: {
				activeElement: null,
				createElement: vi
					.fn<
						(
							tagName: string,
						) => typeof status | typeof progress | typeof progressIndicator | typeof findRoot
					>()
					.mockReturnValueOnce(status)
					.mockReturnValueOnce(progress)
					.mockReturnValueOnce(progressIndicator)
					.mockReturnValueOnce(findRoot),
				hasFocus: vi.fn<() => boolean>(() => false),
			},
			style: {},
		} as unknown as HTMLElement;
		const documentElement = { dir: '', lang: '' };
		const applyFrame = vi.fn<ProjectedTerminal['applyFrame']>(() => 'committed');
		const applyInputCapability = vi
			.fn<ProjectedTerminal['applyInputCapability']>()
			.mockReturnValue({ kind: 'applied' });
		const applyRows = vi.fn<ProjectedTerminal['applyRows']>(() => 'committed');
		const captureActionContext = vi.fn<ProjectedTerminal['captureActionContext']>(() => ({
			kind: 'product',
			link: null,
		}));
		const clearTerminal = vi.fn<ProjectedTerminal['clear']>(() => true);
		const notifyLinkOpened = vi.fn<ProjectedTerminal['notifyLinkOpened']>();
		let emitBell: Parameters<ProjectedTerminal['onBell']>[0] = () => undefined;
		const notifyBell = vi.fn<ProjectedTerminal['notifyBell']>((event) => emitBell(event));
		let emitClipboardWriteRequest: Parameters<
			ProjectedTerminal['onClipboardWriteRequest']
		>[0] = () => undefined;
		const notifyClipboardWriteRequest = vi.fn<ProjectedTerminal['notifyClipboardWriteRequest']>(
			(event) => emitClipboardWriteRequest(event),
		);
		const notifyExited = vi.fn<ProjectedTerminal['notifyExited']>();
		const notifyFailed = vi.fn<ProjectedTerminal['notifyFailed']>();
		let emitError: Parameters<ProjectedTerminal['onError']>[0] = () => undefined;
		const notifyBrokerFailure = vi.fn<ProjectedTerminal['notifyBrokerFailure']>((failure) =>
			emitError({ ...failure, error: new Error(failure.message) }),
		);
		const notifySelectionCopied = vi.fn<ProjectedTerminal['notifySelectionCopied']>();
		const disposeTerminal = vi.fn<ProjectedTerminal['dispose']>();
		const freezeTerminal = vi.fn<ProjectedTerminal['freeze']>();
		const focusTerminal = vi.fn<ProjectedTerminal['focus']>();
		const openTerminal = vi.fn<ProjectedTerminal['open']>().mockResolvedValue(undefined);
		const pasteTerminal = vi.fn<ProjectedTerminal['paste']>().mockResolvedValue({ kind: 'empty' });
		const setResizeOwnership = vi.fn<ProjectedTerminal['setResizeOwnership']>();
		const setOptionAsAltMode = vi.fn<ProjectedTerminal['setOptionAsAltMode']>();
		const setCopyOnSelect = vi.fn<ProjectedTerminal['setCopyOnSelect']>();
		const updatePresentationConfiguration = vi
			.fn<ProjectedTerminal['updatePresentationConfiguration']>()
			.mockResolvedValue({
				configuration: DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
				kind: 'completed',
			});
		const writeClipboardText = vi.fn<
			NonNullable<ProjectedTerminalPlatformEffects['writeClipboardText']>
		>(async () => ({ kind: 'completed' as const }));
		const isClipboardWriteAvailable = vi.fn<
			NonNullable<ProjectedTerminalPlatformEffects['isClipboardWriteAvailable']>
		>(() => false);
		const playBellSound = vi.fn<NonNullable<ProjectedTerminalPlatformEffects['playBellSound']>>();
		const showBellNotification =
			vi.fn<NonNullable<ProjectedTerminalPlatformEffects['showBellNotification']>>();
		let emitExit: Parameters<ProjectedTerminal['onExit']>[0] = () => undefined;
		let emitFrameTelemetry: Parameters<ProjectedTerminal['onFrameTelemetry']>[0] = () => undefined;
		let emitLinkOpenRequest: Parameters<ProjectedTerminal['onLinkOpenRequest']>[0] = () =>
			undefined;
		let emitMetadata: Parameters<ProjectedTerminal['onMetadataChange']>[0] | undefined;
		let emitSelectionCopyResult: Parameters<ProjectedTerminal['onSelectionCopyResult']>[0] = () =>
			undefined;
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
						applyInputCapability,
						applyRows,
						canControlTerminal: true,
						captureActionContext,
						clear: clearTerminal,
						clearSearch: vi.fn<ProjectedTerminal['clearSearch']>(),
						copySelectionAsync: vi
							.fn<ProjectedTerminal['copySelectionAsync']>()
							.mockResolvedValue({ kind: 'completed' }),
						dispose: disposeTerminal,
						findNext: vi.fn<ProjectedTerminal['findNext']>(() => false),
						findPrevious: vi.fn<ProjectedTerminal['findPrevious']>(() => false),
						focus: focusTerminal,
						freeze: freezeTerminal,
						getBufferRowsSnapshot: vi.fn<ProjectedTerminal['getBufferRowsSnapshot']>(() => ({
							generation: 1n,
							revision: 0n,
							totalRows: 0n,
						})),
						linkOpenRequestIsCurrent: vi.fn<ProjectedTerminal['linkOpenRequestIsCurrent']>(
							() => true,
						),
						notifyExited,
						notifyFailed,
						notifyBell,
						notifyClipboardWriteRequest,
						notifyBrokerFailure,
						notifyHistoryChanged: vi.fn<ProjectedTerminal['notifyHistoryChanged']>(),
						notifyLinkOpened,
						notifySelectionChanged: vi.fn<ProjectedTerminal['notifySelectionChanged']>(),
						notifySelectionCopied,
						onError: vi.fn<ProjectedTerminal['onError']>((listener) => {
							emitError = listener as (event: ProjectedTerminalErrorEvent) => void;
							return { dispose: vi.fn<() => void>() };
						}),
						onBell: vi.fn<ProjectedTerminal['onBell']>((listener) => {
							emitBell = listener;
							return { dispose: vi.fn<() => void>() };
						}),
						onClipboardWriteRequest: vi.fn<ProjectedTerminal['onClipboardWriteRequest']>(
							(listener) => {
								emitClipboardWriteRequest = listener;
								return { dispose: vi.fn<() => void>() };
							},
						),
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
						onSelectionCopyResult: vi.fn<ProjectedTerminal['onSelectionCopyResult']>((listener) => {
							emitSelectionCopyResult = listener;
							return { dispose: vi.fn<() => void>() };
						}),
						onSelectionCopied: vi.fn<ProjectedTerminal['onSelectionCopied']>(() => ({
							dispose: vi.fn<() => void>(),
						})),
						openLinkAsync: vi
							.fn<ProjectedTerminal['openLinkAsync']>()
							.mockResolvedValue({ kind: 'completed' }),
						open: openTerminal,
						paste: pasteTerminal,
						reset: vi.fn<ProjectedTerminal['reset']>(),
						scrollToBottom: vi.fn<ProjectedTerminal['scrollToBottom']>(() => true),
						selectAll: vi.fn<ProjectedTerminal['selectAll']>(),
						selection: {
							attachmentId: bootstrap.attachmentId,
							end: { column: 1, row: CanonicalUint64TextSchema.parse('0') },
							preview: 'selected',
							rectangle: false,
							start: { column: 0, row: CanonicalUint64TextSchema.parse('0') },
							state: 'active',
						},
						setResizeOwnership,
						setCopyOnSelect,
						setOptionAsAltMode,
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
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
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
			theme,
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
		let emitInputConfiguration: Parameters<TerminalInputConfigurationSource['subscribe']>[0] = () =>
			undefined;
		const unsubscribeInputConfiguration = vi.fn<() => void>();
		const inputConfigurationSource: TerminalInputConfigurationSource = {
			current: () => ({ optionAsAlt: 'right' }),
			subscribe: (listener) => {
				emitInputConfiguration = listener;
				return unsubscribeInputConfiguration;
			},
		};
		let emitInteractionConfiguration: Parameters<
			TerminalInteractionConfigurationSource['subscribe']
		>[0] = () => undefined;
		const unsubscribeInteractionConfiguration = vi.fn<() => void>();
		const interactionConfigurationSource: TerminalInteractionConfigurationSource = {
			current: () => ({ copyOnSelect: false }),
			subscribe: (listener) => {
				emitInteractionConfiguration = listener;
				return unsubscribeInteractionConfiguration;
			},
		};
		const observeMetadata = vi.fn<(metadata: ProjectedTerminalMetadata) => void>();
		const actionRuntime = new ActionRuntime();
		const actionScope = actionRuntime.createScope('test.workbench.terminal');
		const admitBellEffect = vi.fn<(event: WorkbenchTerminalBellEffect) => boolean>(() => true);
		let contextProvider: WorkbenchTerminalActionContextProvider | undefined;
		const disposeContextProvider = vi.fn<() => void>();
		const bindContextProvider = vi.fn<
			(provider: WorkbenchTerminalActionContextProvider) => { readonly dispose: () => void }
		>((provider) => {
			contextProvider = provider;
			return { dispose: disposeContextProvider };
		});
		const observeCompletion = vi.fn<TerminalCompletionReminderObserver['observeCompletion']>(
			() => ({
				kind: 'remind' as const,
				notification: 'shown' as const,
			}),
		);
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
				{
					isClipboardWriteAvailable,
					openExternal,
					playBellSound,
					showBellNotification,
					writeClipboardText,
				},
				presentationConfigurationSource,
				undefined,
				observeMetadata,
				undefined,
				{ observeCompletion },
				inputConfigurationSource,
				{ admitBellEffect, bindContextProvider, runtime: actionRuntime, scope: actionScope },
				interactionConfigurationSource,
			);
			expect(dependencies.createTerminal).toHaveBeenCalledWith(
				expect.objectContaining({ copyOnSelect: false, projectionBufferTrust: 'local-host' }),
			);
			expect(contextProvider).toBeDefined();
			expect(
				contextProvider?.capture({
					altKey: false,
					clientX: 12,
					clientY: 24,
					ctrlKey: false,
					metaKey: false,
					shiftKey: true,
				}),
			).toMatchObject({ kind: 'product' });
			expect(captureActionContext).toHaveBeenCalledWith({
				clientX: 12,
				clientY: 24,
				shiftKey: true,
			});
			const copyAction = actionRuntime
				.discover({ scope: actionScope, source: 'context-menu' })
				.find(
					(result) => result.kind === 'ready' && result.descriptor.id === 'terminal.selection.copy',
				);
			if (copyAction?.kind !== 'ready') throw new Error('Expected the terminal Copy action.');
			expect(copyAction.availability).toMatchObject({ kind: 'disabled' });
			await contextProvider?.executePrepared(copyAction.invocation).completion;
			await vi.waitFor(() => expect(status.textContent).toBe('此终端操作当前不可用。'));
			const clearAction = actionRuntime
				.discover({ scope: actionScope, source: 'context-menu' })
				.find((result) => result.kind === 'ready' && result.descriptor.id === 'terminal.clear');
			if (clearAction?.kind !== 'ready') throw new Error('Expected the terminal Clear action.');
			await contextProvider?.executePrepared(clearAction.invocation).completion;
			expect(clearTerminal).toHaveBeenCalledOnce();
			await vi.waitFor(() => expect(status.textContent).toBe('已清除终端输出。'));
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
			expect(admitBellEffect).toHaveBeenCalledWith({
				sequence: '5',
				terminalId: bootstrap.terminalId,
			});
			expect(playBellSound).toHaveBeenCalledWith({
				sequence: '5',
				terminalId: bootstrap.terminalId,
			});
			expect(showBellNotification).toHaveBeenCalledWith({
				sequence: '5',
				terminalId: bootstrap.terminalId,
			});
			expect(dataset['projectedTerminalBellActive']).toBe('true');
			expect(dataset['projectedTerminalBellPhase']).toBe('a');
			expect(dataset['projectedTerminalBellSequence']).toBe('5');
			admitBellEffect.mockReturnValueOnce(false);
			emitTransportMessage({
				event: { attachmentId: bootstrap.attachmentId, sequence: '6' },
				kind: 'bell',
			});
			expect(playBellSound).toHaveBeenCalledOnce();
			expect(showBellNotification).toHaveBeenCalledOnce();
			expect(dataset['projectedTerminalBellPhase']).toBe('a');
			expect(dataset['projectedTerminalBellSequence']).toBe('5');
			emitTransportMessage({
				event: { attachmentId: bootstrap.attachmentId, sequence: '7' },
				kind: 'bell',
			});
			expect(playBellSound).toHaveBeenCalledTimes(2);
			expect(showBellNotification).toHaveBeenCalledTimes(2);
			expect(dataset['projectedTerminalBellPhase']).toBe('b');
			expect(dataset['projectedTerminalBellSequence']).toBe('7');
			emitTransportMessage({
				event: {
					attachmentId: bootstrap.attachmentId,
					sequence: '6',
					text: 'automatic OSC 52 text',
				},
				kind: 'clipboard-write-request',
			});
			expect(notifyClipboardWriteRequest).toHaveBeenCalledWith({
				attachmentId: bootstrap.attachmentId,
				sequence: '6',
				text: 'automatic OSC 52 text',
			});
			await vi.waitFor(() =>
				expect(writeClipboardText).toHaveBeenCalledWith('automatic OSC 52 text'),
			);
			expect(transport.send).toHaveBeenCalledWith({
				kind: 'clipboard-write-decision',
				payload: {
					approved: true,
					attachmentId: bootstrap.attachmentId,
					sequence: '6',
					text: 'automatic OSC 52 text',
				},
			});
			emitLinkOpenRequest({
				context: {
					column: 0,
					generation: '1',
					linkEpoch: '1',
					revision: '1',
					row: '1',
				},
				target: { kind: 'url', url: 'https://example.com/from-osc-8' },
			});
			await vi.waitFor(() =>
				expect(openExternal).toHaveBeenCalledWith('https://example.com/from-osc-8'),
			);
			expect(notifyLinkOpened).toHaveBeenCalledWith('9');
			await vi.waitFor(() => expect(writeClipboardText).toHaveBeenCalledWith('copied from Host'));
			expect(notifySelectionCopied).toHaveBeenCalledWith(16, '10');
			expect(notifyBrokerFailure).not.toHaveBeenCalled();
			await Promise.resolve();
			writeClipboardText.mockResolvedValueOnce({
				code: 'clipboard-denied',
				kind: 'failed',
				retryable: false,
			});
			emitTransportMessage({
				event: {
					attachmentId: bootstrap.attachmentId,
					sequence: '7',
					text: 'failed OSC 52 text',
				},
				kind: 'clipboard-write-request',
			});
			await vi.waitFor(() =>
				expect(notifyBrokerFailure).toHaveBeenCalledWith({
					code: 'clipboard-denied',
					message: 'The terminal clipboard write did not complete.',
					operation: 'clipboard',
					retryable: false,
				}),
			);
			expect(transport.send).toHaveBeenCalledWith({
				kind: 'clipboard-write-decision',
				payload: {
					approved: false,
					attachmentId: bootstrap.attachmentId,
					sequence: '7',
					text: 'failed OSC 52 text',
				},
			});
			expect(status).toMatchObject({
				hidden: false,
				textContent: '无法将终端文本写入剪贴板。',
			});
			await Promise.resolve();
			const writeCountBeforeNextRequest = writeClipboardText.mock.calls.length;
			emitTransportMessage({
				event: {
					attachmentId: bootstrap.attachmentId,
					sequence: '8',
					text: 'next OSC 52 text',
				},
				kind: 'clipboard-write-request',
			});
			await vi.waitFor(() =>
				expect(writeClipboardText).toHaveBeenCalledTimes(writeCountBeforeNextRequest + 1),
			);
			expect(writeClipboardText).toHaveBeenLastCalledWith('next OSC 52 text');
			expect(transport.send).toHaveBeenCalledWith({
				kind: 'clipboard-write-decision',
				payload: {
					approved: true,
					attachmentId: bootstrap.attachmentId,
					sequence: '8',
					text: 'next OSC 52 text',
				},
			});
			emitTransportMessage({ kind: 'invalid-event' });
			expect(dataset['projectedTerminalState']).toBe('failed');
			expect(status).toMatchObject({
				hidden: false,
				textContent: '终端连接失败。',
			});
			emitTransportMessage({ frame: new Uint8Array([4, 5, 6]).buffer, kind: 'frame' });
			expect(dataset['projectedTerminalState']).toBe('open');
			expect(status).toMatchObject({ hidden: true, textContent: null });
			emitSelectionCopyResult({
				requestSequence: CanonicalUint64TextSchema.parse('11'),
				result: { code: 'clipboard-denied', kind: 'failed', retryable: false },
				source: 'automatic',
			});
			expect(status).toMatchObject({ hidden: false, textContent: '无法复制终端所选内容。' });
			expect(dataset['projectedTerminalCopySource']).toBe('automatic');
			expect(dataset['projectedTerminalCopyState']).toBe('failed');
			emitSelectionCopyResult({
				requestSequence: CanonicalUint64TextSchema.parse('12'),
				result: { byteLength: 16, kind: 'completed' },
				source: 'manual',
			});
			expect(status).toMatchObject({ hidden: false, textContent: '已复制 16 字节。' });
			expect(status.setAttribute).toHaveBeenLastCalledWith('role', 'status');
			expect(dataset['projectedTerminalCopySource']).toBe('manual');
			expect(dataset['projectedTerminalCopyState']).toBe('completed');
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
			await expect(session.presentationReady).resolves.toBeUndefined();
			expect(session.isTransportAvailable()).toBe(true);
			if (emitMetadata === undefined) throw new Error('metadata listener is missing');
			const baseMetadata = {
				cellPixelHeight: 18,
				cellPixelWidth: 9,
				columns: 80,
				cursor: {
					blinking: false,
					column: 4,
					row: 2,
					shape: 'block',
					visible: true,
					wideTail: false,
				},
				defaultColors: {
					backgroundRgb: 0x12_34_56,
					cursorRgb: null,
					foregroundRgb: 0xfe_dc_ba,
				},
				progress: { state: 'remove', value: null },
				rows: 24,
				themeRevision: 0n,
			} as ProjectedTerminalMetadata;
			emitMetadata({ ...baseMetadata, progress: { state: 'set', value: 42 } });
			expect(observeMetadata).toHaveBeenCalledWith(
				expect.objectContaining({
					cellPixelHeight: 18,
					cellPixelWidth: 9,
					columns: 80,
					rows: 24,
				}),
			);
			expect(dataset['projectedTerminalCursorBlinking']).toBe('false');
			expect(dataset['projectedTerminalCursorShape']).toBe('block');
			expect(progress.hidden).toBe(false);
			expect(progress.dataset).toMatchObject({ state: 'set' });
			expect(progress.setAttribute).toHaveBeenCalledWith('aria-valuenow', '42');
			expect(progress.style.setProperty).toHaveBeenCalledWith(
				'--ob-comp-projected-terminal-progress-value',
				'42%',
			);
			emitMetadata({ ...baseMetadata, progress: { state: 'indeterminate', value: null } });
			expect(progress.hidden).toBe(false);
			expect(progress.dataset).toMatchObject({ state: 'indeterminate' });
			emitMetadata({ ...baseMetadata, progress: { state: 'error', value: 73 } });
			expect(progress.dataset).toMatchObject({ state: 'error' });
			expect(progress.setAttribute).toHaveBeenCalledWith('aria-valuenow', '73');
			emitMetadata({ ...baseMetadata, progress: { state: 'pause', value: null } });
			expect(progress.dataset).toMatchObject({ state: 'pause' });
			emitMetadata(baseMetadata);
			expect(progress.hidden).toBe(true);
			expect(progress.removeAttribute).toHaveBeenCalledWith('aria-valuenow');
			emitMetadata({
				...baseMetadata,
				commandCompletion: {
					completedAtMilliseconds: 1_786_947_200_000n,
					durationMilliseconds: 8_000n,
					exitCode: 3,
					sequence: 9n,
				},
			});
			expect(observeCompletion).toHaveBeenCalledWith(
				expect.objectContaining({
					focused: false,
					terminalIdentity: `${bootstrap.hostEpoch}:${bootstrap.terminalId}`,
				}),
			);
			expect(dataset['projectedTerminalCommandCompletionObservedSequence']).toBe('9');
			expect(dataset['projectedTerminalCommandReminderDecision']).toBe('remind-shown');
			emitMetadata({
				...baseMetadata,
				commandCompletion: {
					completedAtMilliseconds: 1_786_947_200_000n,
					durationMilliseconds: 8_000n,
					exitCode: 3,
					sequence: 9n,
				},
			});
			expect(observeCompletion).toHaveBeenCalledOnce();
			expect(status.textContent).toContain('命令已在 8 秒后完成');
			expect(dataset['projectedTerminalCommandCompletionSequence']).toBe('9');
			expect(dataset['projectedTerminalCommandNotification']).toBe('shown');
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
			expect(options?.inputConfiguration).toEqual({ optionAsAlt: 'right' });
			emitInputConfiguration({ optionAsAlt: 'both' });
			expect(setOptionAsAltMode).toHaveBeenCalledWith('both');
			emitInteractionConfiguration({ copyOnSelect: true });
			expect(setCopyOnSelect).toHaveBeenCalledWith(true);
			expect(openTerminal).toHaveBeenCalledOnce();
			expect(focusTerminal).not.toHaveBeenCalled();
			expect(root.style.backgroundColor).toBe('#123456');
			expect(dataset['projectedTerminalBackgroundOpacity']).toBe('1');
			expect(options?.inputEnabled).toBe(false);
			expect(notifyExited).toHaveBeenCalledWith(bootstrap.terminalOutcome.event);
			expect(dataset['projectedTerminalAttachmentId']).toBe(bootstrap.attachmentId);
			expect(dataset['projectedTerminalState']).toBe('exited');
			expect(dataset['projectedTerminalCommittedFrames']).toBe('2');
			expect(dataset['projectedTerminalFinalFramePresented']).toBe('true');
			expect(dataset['projectedTerminalRenderedGeneration']).toBe('1');
			expect(dataset['projectedTerminalRenderedRevision']).toBe('1');
			const runtimeConfiguration: TerminalPresentationConfiguration = {
				...DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION,
				allowTransparency: true,
				backgroundOpacity: 0.6,
				defaultColorOverride: {
					background: '#1e1e1e',
					foreground: '#d4d4d4',
				},
				fontFamily: 'Injected Runtime Mono',
			};
			updatePresentationConfiguration.mockResolvedValueOnce({
				configuration: runtimeConfiguration,
				kind: 'completed',
			});
			emitPresentationConfiguration(runtimeConfiguration);
			await vi.waitFor(() =>
				expect(updatePresentationConfiguration).toHaveBeenCalledWith(runtimeConfiguration),
			);
			await vi.waitFor(() => {
				expect(dataset['projectedTerminalPresentationState']).toBe('completed');
				expect(dataset['projectedTerminalPresentationCommittedEpoch']).toBe('1');
			});
			expect(root.style.backgroundColor).toBe('rgba(18, 52, 86, 0.6)');
			expect(dataset['projectedTerminalBackgroundOpacity']).toBe('0.6');
			emitFrameTelemetry({ ...telemetry, frameId: 2, timestamp: 2 });
			expect(dataset['projectedTerminalPresentationDrawnEpoch']).toBe('1');
			expect(dataset['projectedTerminalRenderedFrames']).toBe('2');
			expect(dependencies.createTerminal).toHaveBeenCalledOnce();
			expect(openTerminal).toHaveBeenCalledOnce();
			session.focus();
			expect(focusTerminal).toHaveBeenCalledOnce();
			expect(options?.renderer).toEqual(initialPresentationConfiguration);
			if (emitTransportUnavailable === undefined) {
				throw new Error('transport unavailable listener is missing');
			}
			emitTransportUnavailable();
			expect(dataset['projectedTerminalState']).toBe('exited');
			expect(status).toMatchObject({
				dataset: { projectedTerminalStatus: 'true' },
				hidden: false,
				textContent: '命令已在 8 秒后完成，退出码为 3。',
			});
			session.freeze();
			session.freeze();
			session.focus();
			expect(session.isTransportAvailable()).toBe(false);
			expect(focusTerminal).toHaveBeenCalledOnce();
			expect(dataset['projectedTerminalBellActive']).toBeUndefined();
			expect(freezeTerminal).toHaveBeenCalledOnce();
			expect(disposeTerminal).not.toHaveBeenCalled();
			session.dispose();
			expect(disposeContextProvider).toHaveBeenCalledOnce();
			actionRuntime.dispose();
			session.dispose();
			expect(unsubscribePresentationConfiguration).toHaveBeenCalledOnce();
			expect(unsubscribeInputConfiguration).toHaveBeenCalledOnce();
			expect(unsubscribeInteractionConfiguration).toHaveBeenCalledOnce();
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
				const additionalProgressIndicator = {
					setAttribute: vi.fn<Element['setAttribute']>(),
				};
				const additionalProgress = {
					appendChild: vi.fn<Node['appendChild']>(),
					dataset: {},
					hidden: false,
					remove: vi.fn<ChildNode['remove']>(),
					removeAttribute: vi.fn<Element['removeAttribute']>(),
					setAttribute: vi.fn<Element['setAttribute']>(),
					style: {
						removeProperty: vi.fn<CSSStyleDeclaration['removeProperty']>(),
						setProperty: vi.fn<CSSStyleDeclaration['setProperty']>(),
					},
				};
				const additionalRoot = {
					appendChild: vi.fn<Node['appendChild']>(),
					contains: vi.fn<HTMLElement['contains']>(() => true),
					dataset: {} as DOMStringMap,
					ownerDocument: {
						createElement: vi
							.fn<
								(
									tagName: string,
								) =>
									| typeof additionalStatus
									| typeof additionalProgress
									| typeof additionalProgressIndicator
									| typeof additionalFindRoot
							>()
							.mockReturnValueOnce(additionalStatus)
							.mockReturnValueOnce(additionalProgress)
							.mockReturnValueOnce(additionalProgressIndicator)
							.mockReturnValueOnce(additionalFindRoot),
					},
					style: {},
				} as unknown as HTMLElement;
				Object.setPrototypeOf(additionalRoot, globalThis.HTMLElement.prototype);
				return { additionalFindRoot, additionalRoot, additionalStatus };
			};
			const createAdditionalTransport = () => {
				const close = vi.fn<() => void>();
				let emitMessage: (value: unknown) => void = () => undefined;
				let emitUnavailable: () => void = () => undefined;
				const additionalTransport: ProjectedTerminalTransport = {
					close,
					projectionBufferTrust: 'local-host',
					send: vi.fn<ProjectedTerminalTransport['send']>(),
					subscribe: vi.fn<ProjectedTerminalTransport['subscribe']>((listener, unavailable) => {
						emitMessage = listener;
						emitUnavailable = unavailable;
						return vi.fn<() => void>();
					}),
				};
				return {
					additionalTransport,
					close,
					emitMessage: (value: unknown) => emitMessage(value),
					emitUnavailable: () => emitUnavailable(),
				};
			};
			const liveBootstrap = {
				attachmentId: bootstrap.attachmentId,
				canInput: bootstrap.canInput,
				canResize: bootstrap.canResize,
				capabilityRevision: bootstrap.capabilityRevision,
				creationId: bootstrap.creationId,
				hostEpoch: bootstrap.hostEpoch,
				terminalOutcome: null,
				terminalId: bootstrap.terminalId,
				theme: bootstrap.theme,
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
			let resolvePendingClipboardWrite:
				| ((result: { readonly kind: 'completed' }) => void)
				| undefined;
			const pendingClipboardWrite = vi.fn<
				NonNullable<ProjectedTerminalPlatformEffects['writeClipboardText']>
			>(
				() =>
					new Promise<{ readonly kind: 'completed' }>((resolve) => {
						resolvePendingClipboardWrite = resolve;
					}),
			);
			const liveSession = await startProjectedTerminalRenderer(
				liveElements.additionalRoot,
				documentElement,
				['en-US'],
				'apple',
				liveBootstrap,
				liveTransport.additionalTransport,
				dependencies,
				{ writeClipboardText: pendingClipboardWrite },
				liveSource,
				undefined,
				undefined,
				undefined,
			);
			expect(liveSession.isTransportAvailable()).toBe(true);
			expect(liveElements.additionalRoot.dataset['projectedTerminalState']).toBe('open');
			expect(liveElements.additionalRoot.dataset['projectedTerminalInputCapability']).toBe(
				'read-only',
			);
			expect(liveElements.additionalRoot.dataset['projectedTerminalCapabilityRevision']).toBe('1');
			liveTransport.emitMessage({
				event: {
					attachmentId: liveBootstrap.attachmentId,
					sequence: '1',
					text: 'client-owned pending OSC 52 text',
				},
				kind: 'clipboard-write-request',
			});
			expect(pendingClipboardWrite).toHaveBeenCalledExactlyOnceWith(
				'client-owned pending OSC 52 text',
			);
			expect(liveTransport.additionalTransport.send).not.toHaveBeenCalledWith({
				kind: 'clipboard-write-decision',
				payload: {
					approved: true,
					attachmentId: liveBootstrap.attachmentId,
					sequence: '1',
					text: 'client-owned pending OSC 52 text',
				},
			});
			liveTransport.emitMessage({
				event: {
					attachmentId: liveBootstrap.attachmentId,
					resizeLease: true,
					terminalId: liveBootstrap.terminalId,
				},
				kind: 'resize-lease-changed',
			});
			expect(setResizeOwnership).toHaveBeenCalledWith(true);
			liveTransport.emitMessage({
				capabilities: {
					attachmentId: liveBootstrap.attachmentId,
					capabilities: { input: true, read: true, resize: false },
					capabilityRevision: '2',
					resizeLease: false,
					terminalId: liveBootstrap.terminalId,
				},
				kind: 'attachment-input-capability-changed',
			});
			expect(applyInputCapability).toHaveBeenCalledWith({
				attachmentId: liveBootstrap.attachmentId,
				capabilityRevision: '2',
				input: true,
				terminalId: liveBootstrap.terminalId,
			});
			expect(liveElements.additionalRoot.dataset['projectedTerminalInputCapability']).toBe(
				'read-write',
			);
			expect(liveElements.additionalRoot.dataset['projectedTerminalCapabilityRevision']).toBe('2');
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
			const resizeFailure = {
				code: 'resize-backpressure',
				correlationId: '55'.repeat(16),
				message: 'The Host resize queue is full.',
				operation: 'resize' as const,
				retryable: true,
			};
			liveTransport.emitMessage({ failure: resizeFailure, kind: 'failure' });
			expect(notifyBrokerFailure).toHaveBeenLastCalledWith(resizeFailure);
			expect(liveElements.additionalRoot.dataset).toMatchObject({
				projectedTerminalFailureCode: 'resize-backpressure',
				projectedTerminalFailureCorrelationId: '55'.repeat(16),
				projectedTerminalFailureOperation: 'resize',
				projectedTerminalFailureRetryable: 'true',
				projectedTerminalState: 'open',
			});
			expect(liveElements.additionalStatus).toMatchObject({
				hidden: true,
				textContent: null,
			});
			const historyFailure = {
				code: 'history-unavailable',
				message: 'History is unavailable.',
				operation: 'history' as const,
				retryable: false,
			};
			liveTransport.emitMessage({ failure: historyFailure, kind: 'failure' });
			expect(notifyBrokerFailure).toHaveBeenLastCalledWith(historyFailure);
			expect(liveElements.additionalStatus).toMatchObject({ hidden: true, textContent: null });
			const inputFailure = {
				code: 'input-acknowledgement-unavailable',
				message: 'The input acknowledgement was unavailable.',
				operation: 'input' as const,
				requestSequence: '14',
				retryable: false,
			};
			liveTransport.emitMessage({ failure: inputFailure, kind: 'failure' });
			expect(notifyBrokerFailure).toHaveBeenLastCalledWith(inputFailure);
			expect(liveElements.additionalRoot.dataset).toMatchObject({
				projectedTerminalFailureCode: 'input-acknowledgement-unavailable',
				projectedTerminalFailureOperation: 'input',
				projectedTerminalFailureRequestSequence: '14',
				projectedTerminalFailureRetryable: 'false',
			});
			expect(liveElements.additionalStatus).toMatchObject({ hidden: true, textContent: null });
			const directActionFailure = {
				code: 'control-rejected',
				message: 'Control failed.',
				operation: 'control' as const,
				retryable: false,
			};
			liveTransport.emitMessage({ failure: directActionFailure, kind: 'failure' });
			expect(liveElements.additionalStatus).toMatchObject({
				hidden: false,
				textContent: 'The terminal control action could not be confirmed.',
			});
			expect(liveTransport.close).not.toHaveBeenCalled();
			liveTransport.emitMessage({
				failure: {
					code: 'renderer-backpressure',
					message: 'Renderer command exceeded the bounded Main request window.',
					operation: 'protocol',
					retryable: true,
				},
				kind: 'failure',
			});
			await expect(liveSession.transportUnavailable).resolves.toBeUndefined();
			expect(liveSession.isTransportAvailable()).toBe(false);
			const runtimeFailure = {
				failure: { kind: 'pty-read' as const, stage: 'read-output' as const },
				finalRevision: CanonicalUint64TextSchema.parse('1'),
				identity: {
					creationId: liveBootstrap.creationId,
					hostEpoch: liveBootstrap.hostEpoch,
					terminalId: liveBootstrap.terminalId,
				},
				process: { kind: 'observed' as const, result: { code: 1, kind: 'exit-code' as const } },
			};
			liveTransport.emitMessage({ event: runtimeFailure, kind: 'terminal-failed' });
			expect(notifyFailed).toHaveBeenCalledWith(runtimeFailure);
			expect(liveElements.additionalRoot.dataset['projectedTerminalState']).toBe('failed');
			expect(liveElements.additionalStatus).toMatchObject({
				hidden: false,
				textContent: 'The terminal session stopped after a runtime failure.',
			});
			liveSession.dispose();
			resolvePendingClipboardWrite?.({ kind: 'completed' });
			await Promise.resolve();
			await Promise.resolve();
			expect(liveTransport.additionalTransport.send).not.toHaveBeenCalledWith({
				kind: 'clipboard-write-decision',
				payload: {
					approved: true,
					attachmentId: liveBootstrap.attachmentId,
					sequence: '1',
					text: 'client-owned pending OSC 52 text',
				},
			});
			expect(unsubscribeLiveSource).toHaveBeenCalledOnce();
			expect(liveTransport.close).toHaveBeenCalledOnce();

			const hostCloseElements = createAdditionalRoot();
			const hostCloseTransport = createAdditionalTransport();
			const hostCloseSession = await startProjectedTerminalRenderer(
				hostCloseElements.additionalRoot,
				documentElement,
				['en-US'],
				'apple',
				liveBootstrap,
				hostCloseTransport.additionalTransport,
				dependencies,
				{},
				{
					current: () => initialPresentationConfiguration,
					subscribe: () => vi.fn<() => void>(),
				},
			);
			hostCloseTransport.emitMessage({
				event: {
					attachmentId: liveBootstrap.attachmentId,
					sequence: '2',
					text: 'desktop-owned automatic write',
				},
				kind: 'clipboard-write-request',
			});
			expect(hostCloseTransport.additionalTransport.send).toHaveBeenCalledWith({
				kind: 'clipboard-write-decision',
				payload: {
					approved: true,
					attachmentId: liveBootstrap.attachmentId,
					sequence: '2',
					text: 'desktop-owned automatic write',
				},
			});
			hostCloseTransport.emitMessage({
				failure: {
					code: 'host-connection-unavailable',
					message: 'The Terminal Host connection is unavailable.',
					operation: 'protocol',
					retryable: true,
				},
				kind: 'failure',
			});
			await expect(hostCloseSession.transportUnavailable).resolves.toBeUndefined();
			expect(hostCloseSession.isTransportAvailable()).toBe(false);
			hostCloseSession.dispose();

			vi.mocked(dependencies.createTerminal).mockClear();
			disposeTerminal.mockClear();
			openTerminal.mockClear();
			openTerminal.mockImplementationOnce(() => new Promise<void>(() => undefined));
			const startupDisconnectElements = createAdditionalRoot();
			const startupDisconnectTransport = createAdditionalTransport();
			const startup = startProjectedTerminalRenderer(
				startupDisconnectElements.additionalRoot,
				documentElement,
				['en-US'],
				'apple',
				liveBootstrap,
				startupDisconnectTransport.additionalTransport,
				dependencies,
				{},
				{
					current: () => initialPresentationConfiguration,
					subscribe: () => vi.fn<() => void>(),
				},
			);
			await vi.waitFor(() => expect(openTerminal).toHaveBeenCalledOnce());
			startupDisconnectTransport.emitUnavailable();
			await expect(startup).rejects.toThrow(
				'Projected terminal transport became unavailable during renderer startup.',
			);
			expect(disposeTerminal).toHaveBeenCalledOnce();
			expect(startupDisconnectTransport.close).toHaveBeenCalledOnce();
			openTerminal.mockResolvedValue(undefined);

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
