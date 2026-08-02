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
	ProjectedTerminalOptions,
} from '@orchbee/terminal';
import { describe, expect, it, vi } from 'vitest';

import { startDesktopRenderer, type DesktopRendererDependencies } from '../startDesktopRenderer';

describe('startDesktopRenderer', () => {
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
		} as unknown as HTMLElement;
		const documentElement = { dir: '', lang: '' };
		const applyFrame = vi.fn<ProjectedTerminal['applyFrame']>(() => 'committed');
		const applyRows = vi.fn<ProjectedTerminal['applyRows']>(() => 'committed');
		const notifyLinkOpened = vi.fn<ProjectedTerminal['notifyLinkOpened']>();
		const notifyExited = vi.fn<ProjectedTerminal['notifyExited']>();
		let emitExit: Parameters<ProjectedTerminal['onExit']>[0] = () => undefined;
		let emitFrameTelemetry: Parameters<ProjectedTerminal['onFrameTelemetry']>[0] = () => undefined;
		let options: ProjectedTerminalOptions | undefined;
		const dependencies: DesktopRendererDependencies = {
			createTerminal: vi.fn<DesktopRendererDependencies['createTerminal']>((nextOptions) => {
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
					notifyBrokerFailure: vi.fn<ProjectedTerminal['notifyBrokerFailure']>(),
					notifyHistoryChanged: vi.fn<ProjectedTerminal['notifyHistoryChanged']>(),
					notifyLinkOpened,
					notifySelectionChanged: vi.fn<ProjectedTerminal['notifySelectionChanged']>(),
					notifySelectionCopied: vi.fn<ProjectedTerminal['notifySelectionCopied']>(),
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
					onMetadataChange: vi.fn<ProjectedTerminal['onMetadataChange']>(() => ({
						dispose: vi.fn<() => void>(),
					})),
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
			}),
			installShortcuts: vi.fn<DesktopRendererDependencies['installShortcuts']>(() =>
				vi.fn<() => void>(),
			),
			mountFindSurface: vi.fn<DesktopRendererDependencies['mountFindSurface']>(() => ({
				dispose: vi.fn<() => void>(),
			})),
		};
		const channel = new MessageChannel();
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canInput: false,
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalExit: {
				exit: { code: 0, kind: 'exit-code' as const },
				finalRevision: CanonicalUint64TextSchema.parse('1'),
				identity: {
					creationId: CreationIdSchema.parse('22'.repeat(16)),
					hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
					terminalId: TerminalIdSchema.parse('44'.repeat(16)),
				},
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
			const session = await startDesktopRenderer(
				root,
				documentElement,
				['zh-CN'],
				'apple',
				bootstrap,
				channel.port1,
				dependencies,
			);
			channel.port2.postMessage({ frame: new Uint8Array([1, 2, 3]).buffer, kind: 'frame' });
			channel.port2.postMessage({ kind: 'link-opened', requestSequence: '9' });
			await vi.waitFor(() => expect(applyFrame).toHaveBeenCalledOnce());
			await vi.waitFor(() => expect(notifyLinkOpened).toHaveBeenCalledWith('9'));
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
				expect.objectContaining({ input: '终端输入', screen: '终端屏幕' }),
			);
			expect(options?.renderer).toEqual({
				fontFamily: '"Menlo", monospace',
				fontSize: 15,
			});
			expect(options?.inputEnabled).toBe(false);
			expect(notifyExited).toHaveBeenCalledWith(bootstrap.terminalExit);
			expect(dataset['desktopTerminalAttachmentId']).toBe(bootstrap.attachmentId);
			expect(dataset['desktopTerminalState']).toBe('exited');
			expect(dataset['desktopTerminalCommittedFrames']).toBe('1');
			expect(dataset['desktopTerminalFinalFramePresented']).toBe('true');
			expect(dataset['desktopTerminalRenderedRevision']).toBe('1');
			session.dispose();
		} finally {
			Object.defineProperty(globalThis, 'HTMLElement', {
				configurable: true,
				value: originalHTMLElement,
			});
			channel.port2.close();
		}
	});
});
