import { DesktopProjectFolderAcquisitionMainEventSchema } from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopProjectFolderAcquisitionBroker,
	type DesktopProjectFolderAcquisitionBrokerResult,
	type DesktopProjectFolderAcquisitionBrokerRuntime,
} from '../DesktopProjectFolderAcquisitionBroker';

describe('DesktopProjectFolderAcquisitionBroker', () => {
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
	return { selectExistingFolder };
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
