import { DesktopProjectFolderAcquisitionMainEventSchema } from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopProjectFolderAcquisitionBroker,
	type DesktopProjectFolderAcquisitionBrokerResult,
	type DesktopProjectFolderAcquisitionBrokerRuntime,
} from '../DesktopProjectFolderAcquisitionBroker';

describe('DesktopProjectFolderAcquisitionBroker', () => {
	it('retains exact ownership while native fallback is pending and a late ACK arrives', async () => {
		const port = new FakeMessagePort();
		let finishReport: (() => void) | undefined;
		const runtime = {
			...createRuntime(async () => ({ kind: 'selected', path: '/workspace' })),
			createFolder: vi.fn<DesktopProjectFolderAcquisitionBrokerRuntime['createFolder']>(
				async () => ({ kind: 'creation-unknown', path: '/workspace/new' }),
			),
			reportUnacknowledgedCreation: vi.fn(
				() =>
					new Promise<void>((resolve) => {
						finishReport = resolve;
					}),
			),
		};
		const broker = new DesktopProjectFolderAcquisitionBroker(port.asMessagePortMain(), runtime);
		broker.start();
		port.emit({ requestId: 'parent:1', type: 'desktop-project-folder-parent-requested' });
		await vi.waitFor(() => expect(port.messages).toHaveLength(1));
		port.emit({
			attempt: 1,
			name: 'new',
			parentRequestId: 'parent:1',
			requestId: 'create:1',
			type: 'desktop-project-folder-creation-requested',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(2));
		port.emit({ requestId: 'parent:1', type: 'desktop-project-folder-parent-released' });
		expect(runtime.reportUnacknowledgedCreation).toHaveBeenCalledExactlyOnceWith(
			'/workspace/new',
			false,
		);
		port.emit({ requestId: 'create:1', type: 'desktop-project-folder-creation-acknowledged' });
		port.emit({ requestId: 'parent:2', type: 'desktop-project-folder-parent-requested' });
		expect(port.messages.at(-1)).toMatchObject({ code: 'busy', requestId: 'parent:2' });
		finishReport?.();
		await Promise.resolve();
		await Promise.resolve();
		await Promise.resolve();
		port.emit({ requestId: 'parent:3', type: 'desktop-project-folder-parent-requested' });
		await vi.waitFor(() =>
			expect(port.messages.at(-1)).toMatchObject({
				requestId: 'parent:3',
				result: { kind: 'parent-selected' },
			}),
		);
		broker.dispose();
		expect(runtime.createFolder).toHaveBeenCalledTimes(1);
	});

	it('keeps a selected parent for explicit conflict correction and consumes it after one successful mkdir', async () => {
		const port = new FakeMessagePort();
		const createFolder = vi
			.fn<DesktopProjectFolderAcquisitionBrokerRuntime['createFolder']>()
			.mockResolvedValueOnce({ code: 'already-exists', kind: 'failed' })
			.mockResolvedValue({ kind: 'created', path: '/workspace/new' });
		const runtime = {
			...createRuntime(async () => ({ kind: 'selected', path: '/workspace' })),
			createFolder,
		};
		const broker = new DesktopProjectFolderAcquisitionBroker(port.asMessagePortMain(), runtime);
		broker.start();
		port.emit({ requestId: 'parent:1', type: 'desktop-project-folder-parent-requested' });
		await vi.waitFor(() => expect(port.messages).toHaveLength(1));
		const first = {
			attempt: 1,
			name: 'taken',
			parentRequestId: 'parent:1',
			requestId: 'create:1',
			type: 'desktop-project-folder-creation-requested',
		};
		port.emit(first);
		await vi.waitFor(() => expect(port.messages).toHaveLength(2));
		port.emit(first);
		expect(createFolder).toHaveBeenCalledTimes(1);
		port.emit({ ...first, attempt: 2, name: 'new', requestId: 'create:2' });
		await vi.waitFor(() => expect(port.messages).toHaveLength(4));
		expect(createFolder).toHaveBeenLastCalledWith('/workspace', 'new', expect.any(Function));
		port.emit({ requestId: 'create:1', type: 'desktop-project-folder-creation-acknowledged' });
		port.emit({ requestId: 'parent:2', type: 'desktop-project-folder-parent-requested' });
		expect(port.messages.at(-1)).toMatchObject({ code: 'busy', requestId: 'parent:2' });
		port.emit({ requestId: 'create:2', type: 'desktop-project-folder-creation-acknowledged' });
		port.emit({ ...first, attempt: 3, requestId: 'create:3' });
		expect(port.messages.at(-1)).toMatchObject({ code: 'stale-parent' });
		expect(createFolder).toHaveBeenCalledTimes(2);
		broker.dispose();
		expect(runtime.reportUnacknowledgedCreation).not.toHaveBeenCalled();
	});

	it('revokes preparation on retirement but reports a mkdir that has already committed', async () => {
		const port = new FakeMessagePort();
		let finish: ((result: { readonly kind: 'created'; readonly path: string }) => void) | undefined;
		let isCurrent: (() => boolean) | undefined;
		const runtime = {
			...createRuntime(async () => ({ kind: 'selected', path: '/workspace' })),
			createFolder: vi.fn<DesktopProjectFolderAcquisitionBrokerRuntime['createFolder']>(
				(_parent, _name, guard) => {
					isCurrent = guard;
					return new Promise((resolve) => {
						finish = resolve;
					});
				},
			),
		};
		const broker = new DesktopProjectFolderAcquisitionBroker(port.asMessagePortMain(), runtime);
		broker.start();
		port.emit({ requestId: 'parent:1', type: 'desktop-project-folder-parent-requested' });
		await vi.waitFor(() => expect(port.messages).toHaveLength(1));
		port.emit({
			attempt: 1,
			name: 'new',
			parentRequestId: 'parent:1',
			requestId: 'create:1',
			type: 'desktop-project-folder-creation-requested',
		});
		expect(isCurrent?.()).toBe(true);
		broker.dispose();
		expect(isCurrent?.()).toBe(false);
		finish?.({ kind: 'created', path: '/workspace/new' });
		await vi.waitFor(() =>
			expect(runtime.reportUnacknowledgedCreation).toHaveBeenCalledExactlyOnceWith(
				'/workspace/new',
				true,
			),
		);
		expect(port.messages).toHaveLength(1);
	});
	it('publishes one correlated selected result without exposing raw failures', async () => {
		const port = new FakeMessagePort();
		const runtime = createRuntime(async () => ({ kind: 'selected', path: '/workspace/project' }));
		const broker = new DesktopProjectFolderAcquisitionBroker(port.asMessagePortMain(), runtime);
		broker.start();

		port.emit(request('project-folder:request:1'));
		await vi.waitFor(() => expect(port.messages).toHaveLength(1));

		expect(DesktopProjectFolderAcquisitionMainEventSchema.parse(port.messages[0])).toEqual({
			requestId: 'project-folder:request:1',
			result: { kind: 'selected', path: '/workspace/project' },
			type: 'desktop-project-folder-acquisition-completed',
		});
	});

	it('rejects concurrent requests and ignores a native completion after disposal', async () => {
		const port = new FakeMessagePort();
		let finish: ((result: DesktopProjectFolderAcquisitionBrokerResult) => void) | undefined;
		const runtime = createRuntime(
			() =>
				new Promise<DesktopProjectFolderAcquisitionBrokerResult>((resolve) => {
					finish = resolve;
				}),
		);
		const broker = new DesktopProjectFolderAcquisitionBroker(port.asMessagePortMain(), runtime);
		broker.start();
		port.emit(request('project-folder:request:1'));
		port.emit(request('project-folder:request:2'));

		expect(DesktopProjectFolderAcquisitionMainEventSchema.parse(port.messages[0])).toMatchObject({
			code: 'busy',
			requestId: 'project-folder:request:2',
		});
		broker.dispose();
		finish?.({ kind: 'selected', path: '/workspace/late' });
		await Promise.resolve();
		expect(port.messages).toHaveLength(1);
		expect(port.close).toHaveBeenCalledOnce();
	});

	it('retires the capability on malformed input', () => {
		const port = new FakeMessagePort();
		const broker = new DesktopProjectFolderAcquisitionBroker(
			port.asMessagePortMain(),
			createRuntime(async () => ({ kind: 'cancelled' })),
		);
		broker.start();

		port.emit({ type: 'generic-filesystem-request' });

		expect(port.close).toHaveBeenCalledOnce();
	});
});

function request(requestId: string): unknown {
	return { requestId, type: 'desktop-project-folder-selection-requested' };
}

function createRuntime(
	selectExistingFolder: () => Promise<DesktopProjectFolderAcquisitionBrokerResult>,
): DesktopProjectFolderAcquisitionBrokerRuntime {
	return {
		createFolder: vi.fn(async () => ({ code: 'validation-failed', kind: 'failed' }) as const),
		reportUnacknowledgedCreation: vi.fn(async () => undefined),
		selectExistingFolder,
	};
}

class FakeMessagePort {
	readonly #closeListeners = new Set<() => void>();
	readonly close = vi.fn<() => void>();
	readonly messages: unknown[] = [];
	readonly #messageListeners = new Set<(event: MessageEvent) => void>();

	asMessagePortMain(): MessagePortMain {
		return this as unknown as MessagePortMain;
	}

	emit(data: unknown): void {
		for (const listener of this.#messageListeners) {
			listener({ data, ports: [] } as unknown as MessageEvent);
		}
	}

	off(event: string, listener: (...arguments_: never[]) => void): void {
		if (event === 'message') {
			this.#messageListeners.delete(listener as unknown as (event: MessageEvent) => void);
		} else if (event === 'close') {
			this.#closeListeners.delete(listener as unknown as () => void);
		}
	}

	on(event: string, listener: (...arguments_: never[]) => void): void {
		if (event === 'message') {
			this.#messageListeners.add(listener as unknown as (event: MessageEvent) => void);
		} else if (event === 'close') {
			this.#closeListeners.add(listener as unknown as () => void);
		}
	}

	postMessage(message: unknown): void {
		this.messages.push(message);
	}

	start(): void {}
}
