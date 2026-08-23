import {
	DesktopTerminalDefaultSettingsMainEventSchema,
	DesktopTerminalDefaultSettingsSnapshotSchema,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import {
	DesktopTerminalDefaultSettingsBroker,
	type DesktopTerminalDefaultSettingsBrokerController,
} from '../DesktopTerminalDefaultSettingsBroker';

const snapshot = DesktopTerminalDefaultSettingsSnapshotSchema.parse({
	configuration: { scrollbackBytes: 10_000_000 },
	persistenceIssue: null,
	source: 'runtime',
});

describe('DesktopTerminalDefaultSettingsBroker', () => {
	it('returns the current snapshot and routes bounded updates', async () => {
		const port = new FakeMessagePort();
		const controller = createController();
		const broker = new DesktopTerminalDefaultSettingsBroker(port.asMessagePortMain(), controller);
		broker.start();

		port.emit({
			requestId: 'terminal-defaults:request:1',
			type: 'desktop-terminal-default-settings-snapshot-requested',
		});
		await vi.waitFor(() => expect(port.messages).toHaveLength(1));
		port.emit(updateRequest('terminal-defaults:request:2', 20_000_000));
		await vi.waitFor(() => expect(port.messages).toHaveLength(2));

		expect(DesktopTerminalDefaultSettingsMainEventSchema.parse(port.messages[0])).toEqual({
			requestId: 'terminal-defaults:request:1',
			result: { kind: 'completed', snapshot },
			type: 'desktop-terminal-default-settings-responded',
		});
		expect(controller.update).toHaveBeenCalledWith({ scrollbackBytes: 20_000_000 });
	});

	it('rejects concurrent requests and retires on malformed input', async () => {
		const port = new FakeMessagePort();
		let finish:
			| ((result: { readonly kind: 'completed'; readonly snapshot: typeof snapshot }) => void)
			| undefined;
		const controller = createController();
		controller.update.mockImplementationOnce(
			() =>
				new Promise((resolve) => {
					finish = resolve;
				}),
		);
		const broker = new DesktopTerminalDefaultSettingsBroker(port.asMessagePortMain(), controller);
		broker.start();
		port.emit(updateRequest('terminal-defaults:request:1', 20));
		port.emit(updateRequest('terminal-defaults:request:2', 30));

		expect(DesktopTerminalDefaultSettingsMainEventSchema.parse(port.messages[0])).toMatchObject({
			requestId: 'terminal-defaults:request:2',
			result: { code: 'busy', kind: 'failed' },
		});
		finish?.({ kind: 'completed', snapshot });
		await Promise.resolve();
		broker.dispose();

		const malformedPort = new FakeMessagePort();
		const malformedBroker = new DesktopTerminalDefaultSettingsBroker(
			malformedPort.asMessagePortMain(),
			createController(),
		);
		malformedBroker.start();
		malformedPort.emit({ type: 'unknown-terminal-default-settings-command' });
		expect(malformedPort.close).toHaveBeenCalledOnce();
	});
});

function updateRequest(requestId: string, scrollbackBytes: number): unknown {
	return {
		configuration: { scrollbackBytes },
		requestId,
		type: 'desktop-terminal-default-settings-update-requested',
	};
}

function createController(): DesktopTerminalDefaultSettingsBrokerController & {
	readonly update: ReturnType<typeof vi.fn>;
} {
	return {
		getSnapshot: () => snapshot,
		update: vi.fn<DesktopTerminalDefaultSettingsBrokerController['update']>(async () => ({
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
