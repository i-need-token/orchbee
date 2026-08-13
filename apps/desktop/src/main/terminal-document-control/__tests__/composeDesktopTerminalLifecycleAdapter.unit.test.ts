import {
	CanonicalUint64TextSchema,
	CreationIdSchema,
	HostEpochSchema,
	MessageKind,
	TerminalIdSchema,
	type TerminalIdentity,
} from '@orchbee/terminal-protocol';
import {
	terminalPresentationId,
	terminalPresentationOperationId,
} from '@orchbee/workbench/terminal-presentation';
import { describe, expect, it, vi } from 'vitest';

import type { HostConnection } from '../../host';
import type { TerminalBinding, TerminalBindingId, TerminalBindingStore } from '../../recovery';
import { DesktopTerminalBindingRegistry } from '../../terminal-binding-registry';
import { resolveDesktopRuntimeConfiguration } from '../../composition';
import { composeDesktopTerminalLifecycleAdapter } from '../composeDesktopTerminalLifecycleAdapter';

const identity: TerminalIdentity = {
	creationId: CreationIdSchema.parse('11'.repeat(16)),
	hostEpoch: HostEpochSchema.parse('22'.repeat(16)),
	terminalId: TerminalIdSchema.parse('33'.repeat(16)),
};

type TerminalBindingDocument = Extract<
	Awaited<ReturnType<TerminalBindingStore['load']>>,
	{ readonly kind: 'available' }
>['document'];

class MemoryStore implements TerminalBindingStore {
	document: TerminalBindingDocument | undefined;
	readonly writes: TerminalBinding[] = [];

	load() {
		return Promise.resolve(
			this.document === undefined
				? ({ kind: 'missing' } as const)
				: ({ document: this.document, kind: 'available' } as const),
		);
	}

	put(binding: TerminalBinding): Promise<void> {
		const bindings = [
			...(this.document?.bindings.filter(
				(candidate) => candidate.bindingId !== binding.bindingId,
			) ?? []),
			binding,
		].sort((left, right) => left.bindingId.localeCompare(right.bindingId));
		this.document = { bindings, version: 3 };
		this.writes.push(binding);
		return Promise.resolve();
	}

	remove(bindingId: TerminalBindingId): Promise<void> {
		this.document = {
			bindings: this.document?.bindings.filter((binding) => binding.bindingId !== bindingId) ?? [],
			version: 3,
		};
		return Promise.resolve();
	}
}

describe('composeDesktopTerminalLifecycleAdapter', () => {
	it('binds document provisioning to the durable recovery lifecycle', async () => {
		const store = new MemoryStore();
		const requestKinds: number[] = [];
		const requestControl = vi.fn<(...arguments_: unknown[]) => Promise<unknown>>(async (kind) => {
			requestKinds.push(kind as number);
			if (kind === MessageKind.listTerminals) {
				return {
					inventoryGeneration: CanonicalUint64TextSchema.parse('1'),
					nextCursor: null,
					terminals: [],
				};
			}
			if (kind === MessageKind.createTerminal) return identity;
			throw new Error(`Unexpected Host request ${String(kind)}`);
		});
		const host = { hostEpoch: identity.hostEpoch, requestControl } as unknown as HostConnection;
		const registry = new DesktopTerminalBindingRegistry();
		const terminalLaunch = resolveDesktopRuntimeConfiguration({
			applicationPath: '/repo/apps/desktop',
			environment: { SHELL: '/bin/sh' },
			homeDirectory: '/users/orchbee',
			platform: 'linux',
			posixRuntimeBaseDirectory: '/run/user/501',
			terminalHostName: 'orchbee-host',
			userDataPath: '/users/orchbee/.config/orchbee',
		}).terminalLaunch;
		const adapter = composeDesktopTerminalLifecycleAdapter({
			allocateFreshBindingId: () => 'binding-1',
			host,
			randomCreationId: () => identity.creationId,
			registry,
			retireProjection: async () => undefined,
			store,
			terminalLaunch,
		});

		await expect(
			adapter.execute({
				launchCwd: '/workspace/orchbee',
				operationId: terminalPresentationOperationId('provision-1'),
				presentationId: terminalPresentationId('presentation-1'),
				type: 'terminal-provision-requested',
			}),
		).resolves.toEqual({
			operationId: 'provision-1',
			presentationId: 'presentation-1',
			type: 'terminal-provision-completed',
		});
		expect(requestKinds).toEqual([MessageKind.listTerminals, MessageKind.createTerminal]);
		expect(store.writes.map((binding) => binding.state.kind)).toEqual([
			'planned',
			'create-requested',
			'retained',
		]);
		expect(store.document?.bindings[0]).toMatchObject({
			bindingId: 'binding-1',
			creationId: identity.creationId,
			launch: { currentDirectory: '/workspace/orchbee', executable: '/bin/sh' },
			state: { identity, kind: 'retained' },
		});
		expect(registry.getByPresentationId(terminalPresentationId('presentation-1'))).toEqual({
			bindingId: 'binding-1',
			presentationId: 'presentation-1',
		});
	});
});
