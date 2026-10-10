import {
	DesktopProjectWorktreeMainEventSchema,
	DesktopProjectWorktreeRendererCommandSchema,
} from '@orchbee/desktop-client-protocol';
import type { MessageEvent, MessagePortMain } from 'electron';
import { describe, expect, it, vi } from 'vitest';

import { DesktopProjectWorktreeBroker } from '../DesktopProjectWorktreeBroker';

type BrokerRuntime = ConstructorParameters<typeof DesktopProjectWorktreeBroker>[1];

describe('DesktopProjectWorktreeBroker', () => {
	it('publishes correlated discovery, creation, and adoption results', async () => {
		const port = new FakeMessagePort();
		const runtime: BrokerRuntime = {
			adopt: vi.fn<BrokerRuntime['adopt']>(async () => ({
				kind: 'completed' as const,
				value: { directoryId: 'discovered-worktree-1', workspaceId: 'workspace-3' },
			})),
			create: vi.fn<BrokerRuntime['create']>(async () => ({
				kind: 'completed' as const,
				value: { directoryId: 'managed-worktree-1', workspaceId: 'workspace-2' },
			})),
			discover: vi.fn<BrokerRuntime['discover']>(async () => ({
				kind: 'completed' as const,
				value: { managedRoot: '/managed/worktrees', repositories: [] },
			})),
		};
		const broker = new DesktopProjectWorktreeBroker(port.asMessagePortMain(), runtime);
		broker.start();

		port.emit(discoveryRequest('worktree-request-1'));
		await vi.waitFor(() => expect(port.messages).toHaveLength(1));
		expect(DesktopProjectWorktreeMainEventSchema.parse(port.messages[0])).toMatchObject({
			discovery: { managedRoot: '/managed/worktrees' },
			requestId: 'worktree-request-1',
			type: 'desktop-project-worktree-discovery-completed',
		});

		port.emit(creationRequest('worktree-request-2'));
		await vi.waitFor(() => expect(port.messages).toHaveLength(2));
		expect(DesktopProjectWorktreeMainEventSchema.parse(port.messages[1])).toMatchObject({
			requestId: 'worktree-request-2',
			result: { directoryId: 'managed-worktree-1', workspaceId: 'workspace-2' },
			type: 'desktop-project-worktree-creation-completed',
		});

		port.emit(adoptionRequest('worktree-request-3'));
		await vi.waitFor(() => expect(port.messages).toHaveLength(3));
		expect(DesktopProjectWorktreeMainEventSchema.parse(port.messages[2])).toMatchObject({
			requestId: 'worktree-request-3',
			result: { directoryId: 'discovered-worktree-1', workspaceId: 'workspace-3' },
			type: 'desktop-project-worktree-adoption-completed',
		});
	});

	it('rejects a concurrent request and suppresses completion after retirement', async () => {
		const port = new FakeMessagePort();
		let finish: (() => void) | undefined;
		const runtime: BrokerRuntime = {
			adopt: vi.fn<BrokerRuntime['adopt']>(),
			create: vi.fn<BrokerRuntime['create']>(
				() =>
					new Promise<{ readonly code: 'creation-failed'; readonly kind: 'failed' }>((resolve) => {
						finish = () => resolve({ code: 'creation-failed', kind: 'failed' });
					}),
			),
			discover: vi.fn<BrokerRuntime['discover']>(async () => ({
				code: 'discovery-failed' as const,
				kind: 'failed' as const,
			})),
		};
		const broker = new DesktopProjectWorktreeBroker(port.asMessagePortMain(), runtime);
		broker.start();
		port.emit(creationRequest('worktree-request-1'));
		port.emit(discoveryRequest('worktree-request-2'));

		expect(DesktopProjectWorktreeMainEventSchema.parse(port.messages[0])).toMatchObject({
			code: 'busy',
			operation: 'discovery',
			requestId: 'worktree-request-2',
		});
		broker.dispose();
		finish?.();
		await Promise.resolve();
		expect(port.messages).toHaveLength(1);
		expect(port.close).toHaveBeenCalledOnce();
	});

	it('retires the capability on malformed input or transferred authority', () => {
		for (const event of [
			{ data: { type: 'generic-git-request' }, ports: [] },
			{ data: discoveryRequest('worktree-request-1'), ports: [{}] },
		]) {
			const port = new FakeMessagePort();
			const broker = new DesktopProjectWorktreeBroker(port.asMessagePortMain(), {
				adopt: vi.fn<BrokerRuntime['adopt']>(),
				create: vi.fn<BrokerRuntime['create']>(),
				discover: vi.fn<BrokerRuntime['discover']>(),
			});
			broker.start();
			port.emitEvent(event);
			expect(port.close).toHaveBeenCalledOnce();
		}
	});
});

function adoptionRequest(requestId: string): unknown {
	return DesktopProjectWorktreeRendererCommandSchema.parse({
		sourceWorkspaceId: 'project-1',
		repositoryId: 'repository-1',
		requestId,
		sourceDirectoryId: 'directory-1',
		type: 'desktop-project-worktree-adoption-requested',
		worktreeId: 'worktree-1',
	});
}

function discoveryRequest(requestId: string): unknown {
	return DesktopProjectWorktreeRendererCommandSchema.parse({
		preferredDirectoryId: null,
		sourceWorkspaceId: 'project-1',
		requestId,
		type: 'desktop-project-worktree-discovery-requested',
	});
}

function creationRequest(requestId: string): unknown {
	return DesktopProjectWorktreeRendererCommandSchema.parse({
		baseRef: 'main',
		branchName: 'orchbee/workspace-1',
		createWorkspace: true,
		sourceWorkspaceId: 'project-1',
		repositoryId: 'repository-1',
		requestId,
		sourceDirectoryId: 'directory-1',
		type: 'desktop-project-worktree-creation-requested',
	});
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
		this.emitEvent({ data, ports: [] });
	}

	emitEvent(event: { readonly data: unknown; readonly ports: readonly unknown[] }): void {
		for (const listener of this.#messageListeners) {
			listener(event as unknown as MessageEvent);
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
