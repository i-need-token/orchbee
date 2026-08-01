import {
	AttachmentIdSchema,
	CreationIdSchema,
	HostEpochSchema,
	TerminalIdSchema,
} from '@orchbee/terminal-protocol';
import type { IpcRendererEvent } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	installProjectedAttachmentBridge,
	type ProjectedAttachmentIpc,
	type ProjectedAttachmentWindow,
} from '../installProjectedAttachmentBridge';

describe('installProjectedAttachmentBridge', () => {
	it('forwards exactly one validated port without exposing a generic IPC function', () => {
		let listener: ((event: IpcRendererEvent, value: unknown) => void) | undefined;
		const ipc: ProjectedAttachmentIpc = {
			once: vi.fn<ProjectedAttachmentIpc['once']>((_channel, nextListener) => {
				listener = nextListener;
			}),
		};
		const targetWindow: ProjectedAttachmentWindow = {
			location: { origin: 'null' },
			postMessage: vi.fn<ProjectedAttachmentWindow['postMessage']>(),
		};
		const port = new MessageChannel().port1;
		const bootstrap = {
			attachmentId: AttachmentIdSchema.parse('11'.repeat(16)),
			canResize: true,
			creationId: CreationIdSchema.parse('22'.repeat(16)),
			hostEpoch: HostEpochSchema.parse('33'.repeat(16)),
			terminalId: TerminalIdSchema.parse('44'.repeat(16)),
		};
		installProjectedAttachmentBridge(ipc, targetWindow, {
			contextIsolated: true,
			sandboxed: true,
		});
		listener?.({ ports: [port] } as IpcRendererEvent, bootstrap);

		const call = vi.mocked(targetWindow.postMessage).mock.calls[0];
		expect(call?.[0]).toEqual(expect.objectContaining({ bootstrap }));
		expect(call?.[0]).toEqual(
			expect.objectContaining({ security: { contextIsolated: true, sandboxed: true } }),
		);
		expect(call?.[1]).toBe('*');
		expect(call?.[2]?.[0]).toBe(port);
	});

	it('drops malformed bootstrap data and ambiguous port sets', () => {
		let listener: ((event: IpcRendererEvent, value: unknown) => void) | undefined;
		const ipc: ProjectedAttachmentIpc = {
			once: (_channel, nextListener) => {
				listener = nextListener;
			},
		};
		const targetWindow: ProjectedAttachmentWindow = {
			location: { origin: 'https://desktop.invalid' },
			postMessage: vi.fn<ProjectedAttachmentWindow['postMessage']>(),
		};
		installProjectedAttachmentBridge(ipc, targetWindow, {
			contextIsolated: true,
			sandboxed: true,
		});
		listener?.({ ports: [] } as unknown as IpcRendererEvent, { arbitrary: true });

		expect(targetWindow.postMessage).not.toHaveBeenCalled();
	});
});
