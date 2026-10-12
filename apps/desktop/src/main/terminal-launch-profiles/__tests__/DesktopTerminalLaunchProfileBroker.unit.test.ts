import {
	DesktopTerminalLaunchProfileMainEventSchema,
	DesktopTerminalLaunchProfileSnapshotSchema,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopTerminalLaunchProfileBroker,
	type DesktopTerminalLaunchProfileBrokerController,
} from '../DesktopTerminalLaunchProfileBroker';
import { createBuiltInTerminalLaunchProfileCatalog } from '../terminal-launch-profile-catalog';

const snapshot = DesktopTerminalLaunchProfileSnapshotSchema.parse({
	capabilities: { loginShell: true },
	catalog: createBuiltInTerminalLaunchProfileCatalog(),
	persistenceIssue: null,
	source: 'built-in',
});

describe('DesktopTerminalLaunchProfileBroker', () => {
	it('returns the current snapshot and routes bounded mutations', async () => {
		const port = new FakeMessagePort();
		const controller = createController();
		const broker = new DesktopTerminalLaunchProfileBroker(port.asMessagePortMain(), controller);
		broker.start();

		port.emit({
			requestId: 'terminal-profile:request:1',
			type: 'desktop-terminal-launch-profile-snapshot-requested',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(1));
		port.emit({
			name: 'Login shell',
			requestId: 'terminal-profile:request:2',
			sourceProfileId: 'system-default',
			type: 'desktop-terminal-launch-profile-create-requested',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(2));

		expect(DesktopTerminalLaunchProfileMainEventSchema.parse(port.messages[0])).toEqual({
			requestId: 'terminal-profile:request:1',
			result: { kind: 'completed', snapshot },
			type: 'desktop-terminal-launch-profile-responded',
		});
		expect(controller.mutate).toHaveBeenCalledWith(
			{
				kind: 'create',
				name: 'Login shell',
				sourceProfileId: 'system-default',
			},
			undefined,
		);
	});

	it('rejects a concurrent request and ignores the active completion after disposal', async () => {
		const port = new FakeMessagePort();
		let finish:
			| ((result: { readonly kind: 'completed'; readonly snapshot: typeof snapshot }) => void)
			| undefined;
		const controller = createController();
		controller.mutate.mockImplementationOnce(
			() =>
				new Promise((resolve) => {
					finish = resolve;
				}),
		);
		const broker = new DesktopTerminalLaunchProfileBroker(port.asMessagePortMain(), controller);
		broker.start();

		port.emit(resetRequest('terminal-profile:request:1'));
		port.emit(resetRequest('terminal-profile:request:2'));

		expect(DesktopTerminalLaunchProfileMainEventSchema.parse(port.messages[0])).toMatchObject({
			requestId: 'terminal-profile:request:2',
			result: { code: 'busy', kind: 'failed' },
		});
		broker.dispose();
		finish?.({ kind: 'completed', snapshot });
		await Promise.resolve();
		expect(port.messages).toHaveLength(1);
		expect(port.close).toHaveBeenCalledOnce();
	});

	it('retires the capability on malformed input or transferred ports', () => {
		const malformedPort = new FakeMessagePort();
		const malformedBroker = new DesktopTerminalLaunchProfileBroker(
			malformedPort.asMessagePortMain(),
			createController(),
		);
		malformedBroker.start();
		malformedPort.emit({ type: 'unknown-terminal-profile-command' });
		expect(malformedPort.close).toHaveBeenCalledOnce();

		const transferredPort = new FakeMessagePort();
		const transferredBroker = new DesktopTerminalLaunchProfileBroker(
			transferredPort.asMessagePortMain(),
			createController(),
		);
		transferredBroker.start();
		transferredPort.emit(resetRequest('terminal-profile:request:3'), [{}]);
		expect(transferredPort.close).toHaveBeenCalledOnce();
	});
});

function resetRequest(requestId: string): unknown {
	return { requestId, type: 'desktop-terminal-launch-profile-reset-requested' };
}

function createController(): DesktopTerminalLaunchProfileBrokerController & {
	readonly mutate: ReturnType<typeof vi.fn>;
} {
	return {
		getSnapshot: () => snapshot,
		mutate: vi.fn<DesktopTerminalLaunchProfileBrokerController['mutate']>(async () => ({
			kind: 'completed',
			snapshot,
		})),
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

	emit(data: unknown, ports: readonly unknown[] = []): void {
		for (const listener of this.#messageListeners) {
			listener({ data, ports } as unknown as MessageEvent);
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
