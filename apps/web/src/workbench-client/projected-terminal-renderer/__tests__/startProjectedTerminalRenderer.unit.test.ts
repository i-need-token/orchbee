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
import { describe, expect, it, vi } from 'vitest';

import type { ProjectedTerminalTransport } from '../../terminal-projection';
import type { ProjectedTerminalPlatformEffects } from '../../projected-terminal-platform-effects';
import {
	startProjectedTerminalRenderer,
	type ProjectedTerminalRendererDependencies,
} from '../startProjectedTerminalRenderer';

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
		const notifyExited = vi.fn<ProjectedTerminal['notifyExited']>();
		const notifyBrokerFailure = vi.fn<ProjectedTerminal['notifyBrokerFailure']>();
		const notifySelectionCopied = vi.fn<ProjectedTerminal['notifySelectionCopied']>();
		const writeClipboardText = vi.fn<
			NonNullable<ProjectedTerminalPlatformEffects['writeClipboardText']>
		>(async () => ({ kind: 'completed' as const }));
		let emitExit: Parameters<ProjectedTerminal['onExit']>[0] = () => undefined;
		let emitFrameTelemetry: Parameters<ProjectedTerminal['onFrameTelemetry']>[0] = () => undefined;
		let emitMetadata: Parameters<ProjectedTerminal['onMetadataChange']>[0] | undefined;
		let options: ProjectedTerminalOptions | undefined;
		const dependencies: ProjectedTerminalRendererDependencies = {
			createTerminal: vi.fn<ProjectedTerminalRendererDependencies['createTerminal']>(
				(nextOptions) => {
					options = nextOptions;
					return {
						applyFrame,
						applyRows,
						clearSearch: vi.fn<ProjectedTerminal['clearSearch']>(),
						copySelectionAsync: vi
							.fn<ProjectedTerminal['copySelectionAsync']>()
							.mockResolvedValue({ kind: 'completed' }),
						dispose: vi.fn<ProjectedTerminal['dispose']>(),
						findNext: vi.fn<ProjectedTerminal['findNext']>(() => false),
						findPrevious: vi.fn<ProjectedTerminal['findPrevious']>(() => false),
						focus: vi.fn<ProjectedTerminal['focus']>(),
						getBufferRowsSnapshot: vi.fn<ProjectedTerminal['getBufferRowsSnapshot']>(() => ({
							generation: 1n,
							revision: 0n,
							totalRows: 0n,
						})),
						notifyExited,
						notifyBrokerFailure,
						notifyHistoryChanged: vi.fn<ProjectedTerminal['notifyHistoryChanged']>(),
						notifyLinkOpened,
						notifySelectionChanged: vi.fn<ProjectedTerminal['notifySelectionChanged']>(),
						notifySelectionCopied,
						onError: vi.fn<ProjectedTerminal['onError']>(() => ({
							dispose: vi.fn<() => void>(),
						})),
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
						onLinkOpenRequest: vi.fn<ProjectedTerminal['onLinkOpenRequest']>(() => ({
							dispose: vi.fn<() => void>(),
						})),
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
						open: vi.fn<ProjectedTerminal['open']>().mockResolvedValue(undefined),
						selectAll: vi.fn<ProjectedTerminal['selectAll']>(),
						selection: { attachmentId: bootstrap.attachmentId, state: 'none' },
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
				{ writeClipboardText },
			);
			expect(dependencies.createTerminal).toHaveBeenCalledWith(
				expect.objectContaining({ projectionBufferTrust: 'local-host' }),
			);
			emitTransportMessage({ frame: new Uint8Array([1, 2, 3]).buffer, kind: 'frame' });
			emitTransportMessage({ kind: 'link-opened', requestSequence: '9' });
			emitTransportMessage({
				content: new TextEncoder().encode('copied from Host').buffer,
				kind: 'selection-copy-content',
				requestSequence: '10',
			});
			expect(applyFrame).toHaveBeenCalledOnce();
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
			expect(options?.renderer).toEqual({
				fontFamily: '"Menlo", monospace',
				fontSize: 15,
				paddingBalance: true,
			});
			expect(root.style.backgroundColor).toBe('#123456');
			expect(options?.inputEnabled).toBe(false);
			expect(notifyExited).toHaveBeenCalledWith(bootstrap.terminalOutcome.event);
			expect(dataset['projectedTerminalAttachmentId']).toBe(bootstrap.attachmentId);
			expect(dataset['projectedTerminalState']).toBe('exited');
			expect(dataset['projectedTerminalCommittedFrames']).toBe('1');
			expect(dataset['projectedTerminalFinalFramePresented']).toBe('true');
			expect(dataset['projectedTerminalRenderedRevision']).toBe('1');
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
		} finally {
			Object.defineProperty(globalThis, 'HTMLElement', {
				configurable: true,
				value: originalHTMLElement,
			});
		}
	});
});
