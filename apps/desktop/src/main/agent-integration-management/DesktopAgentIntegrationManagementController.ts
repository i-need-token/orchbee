import {
	DESKTOP_AGENT_INTEGRATION_IDS,
	DesktopAgentIntegrationManagementSnapshotSchema,
	type DesktopAgentIntegrationId,
	type DesktopAgentIntegrationManagementFailureCode,
	type DesktopAgentIntegrationManagementSnapshot,
	type DesktopAgentIntegrationSnapshotItem,
} from '@orchbee/desktop-client-protocol';

import type {
	AgentIntegrationAdapter,
	AgentIntegrationMutationAttempt,
} from './agent-integration-adapter';

export type DesktopAgentIntegrationManagementOperationResult =
	| { readonly kind: 'completed'; readonly snapshot: DesktopAgentIntegrationManagementSnapshot }
	| {
			readonly code: DesktopAgentIntegrationManagementFailureCode;
			readonly integrationId: DesktopAgentIntegrationId | null;
			readonly kind: 'failed';
			readonly snapshot: DesktopAgentIntegrationManagementSnapshot;
	  };

type LaunchListener = (integrationId: DesktopAgentIntegrationId, revision: number) => void;

export class DesktopAgentIntegrationManagementController {
	#activeOperation = false;
	readonly #adapters: ReadonlyMap<DesktopAgentIntegrationId, AgentIntegrationAdapter>;
	#initialization: Promise<DesktopAgentIntegrationManagementSnapshot> | undefined;
	readonly #launchListeners = new Set<LaunchListener>();
	#launchedIntegrationIds: readonly DesktopAgentIntegrationId[] = Object.freeze([]);
	#revision = 0;
	#snapshot: DesktopAgentIntegrationManagementSnapshot | undefined;

	constructor(adapters: readonly AgentIntegrationAdapter[]) {
		if (
			adapters.length !== DESKTOP_AGENT_INTEGRATION_IDS.length ||
			!DESKTOP_AGENT_INTEGRATION_IDS.every(
				(integrationId, index) => adapters[index]?.integrationId === integrationId,
			)
		) {
			throw new Error('Desktop Agent integration adapters must use canonical order.');
		}
		this.#adapters = new Map(adapters.map((adapter) => [adapter.integrationId, adapter]));
	}

	initialize(): Promise<DesktopAgentIntegrationManagementSnapshot> {
		this.#initialization ??= this.#inspectAll();
		return this.#initialization;
	}

	getSnapshot(): DesktopAgentIntegrationManagementSnapshot {
		if (this.#snapshot === undefined) {
			throw new Error('Desktop Agent integration management must initialize before reading.');
		}
		return this.#snapshot;
	}

	observeLaunch(integrationId: DesktopAgentIntegrationId): boolean {
		if (this.#launchedIntegrationIds.includes(integrationId)) return false;
		this.#launchedIntegrationIds = Object.freeze([...this.#launchedIntegrationIds, integrationId]);
		const revision = this.#revision++;
		if (this.#snapshot !== undefined) {
			this.#snapshot = DesktopAgentIntegrationManagementSnapshotSchema.parse({
				integrations: this.#snapshot.integrations,
				launchedIntegrationIds: this.#launchedIntegrationIds,
				revision,
			});
		}
		for (const listener of this.#launchListeners) listener(integrationId, revision);
		return true;
	}

	subscribeLaunches(listener: LaunchListener): () => void {
		this.#launchListeners.add(listener);
		return () => this.#launchListeners.delete(listener);
	}

	async refresh(): Promise<DesktopAgentIntegrationManagementOperationResult> {
		await this.initialize();
		if (this.#activeOperation) return this.#failure('busy', null);
		this.#activeOperation = true;
		try {
			return { kind: 'completed', snapshot: await this.#inspectAll() };
		} finally {
			this.#activeOperation = false;
		}
	}

	enable(
		integrationId: DesktopAgentIntegrationId,
	): Promise<DesktopAgentIntegrationManagementOperationResult> {
		return this.#mutate(integrationId, 'enable');
	}

	disable(
		integrationId: DesktopAgentIntegrationId,
	): Promise<DesktopAgentIntegrationManagementOperationResult> {
		return this.#mutate(integrationId, 'disable');
	}

	async #mutate(
		integrationId: DesktopAgentIntegrationId,
		operation: 'disable' | 'enable',
	): Promise<DesktopAgentIntegrationManagementOperationResult> {
		await this.initialize();
		if (this.#activeOperation) return this.#failure('busy', integrationId);
		const current = this.#item(integrationId);
		if (current.managementMode === 'manual-only') {
			return this.#failure('manual-action-required', integrationId);
		}
		if (current.state === 'conflict') return this.#failure('conflict', integrationId);
		if (operation === 'enable' && current.availability !== 'available') {
			return this.#failure('agent-unavailable', integrationId);
		}
		if (
			(operation === 'enable' && current.state === 'enabled') ||
			(operation === 'disable' &&
				(current.state === 'disabled' || current.state === 'not-installed'))
		) {
			return { kind: 'completed', snapshot: this.getSnapshot() };
		}
		this.#activeOperation = true;
		try {
			const adapter = this.#adapter(integrationId);
			let attempt: AgentIntegrationMutationAttempt;
			try {
				attempt = await adapter[operation]();
			} catch {
				attempt = { kind: 'failed' };
			}
			const observed = await this.#inspectAdapter(adapter);
			this.#replaceItem(observed);
			const desiredObserved =
				operation === 'enable'
					? observed.state === 'enabled'
					: observed.state === 'disabled' || observed.state === 'not-installed';
			if (desiredObserved) return { kind: 'completed', snapshot: this.getSnapshot() };
			return this.#failure(failureForAttempt(attempt), integrationId);
		} finally {
			this.#activeOperation = false;
		}
	}

	async #inspectAll(): Promise<DesktopAgentIntegrationManagementSnapshot> {
		const integrations = await Promise.all(
			DESKTOP_AGENT_INTEGRATION_IDS.map((integrationId) =>
				this.#inspectAdapter(this.#adapter(integrationId)),
			),
		);
		this.#snapshot = DesktopAgentIntegrationManagementSnapshotSchema.parse({
			integrations,
			launchedIntegrationIds: this.#launchedIntegrationIds,
			revision: this.#revision++,
		});
		return this.#snapshot;
	}

	async #inspectAdapter(
		adapter: AgentIntegrationAdapter,
	): Promise<DesktopAgentIntegrationSnapshotItem> {
		try {
			return await adapter.inspect();
		} catch {
			return {
				availability: 'unknown',
				integrationId: adapter.integrationId,
				issue: 'inspection-failed',
				managementMode: adapter.managementMode,
				state: 'unknown',
			};
		}
	}

	#replaceItem(item: DesktopAgentIntegrationSnapshotItem): void {
		const snapshot = this.getSnapshot();
		this.#snapshot = DesktopAgentIntegrationManagementSnapshotSchema.parse({
			integrations: snapshot.integrations.map((current) =>
				current.integrationId === item.integrationId ? item : current,
			),
			launchedIntegrationIds: this.#launchedIntegrationIds,
			revision: this.#revision++,
		});
	}

	#adapter(integrationId: DesktopAgentIntegrationId): AgentIntegrationAdapter {
		const adapter = this.#adapters.get(integrationId);
		if (adapter === undefined) throw new Error('Unknown Desktop Agent integration adapter.');
		return adapter;
	}

	#item(integrationId: DesktopAgentIntegrationId): DesktopAgentIntegrationSnapshotItem {
		const item = this.getSnapshot().integrations.find(
			(candidate) => candidate.integrationId === integrationId,
		);
		if (item === undefined) throw new Error('Unknown Desktop Agent integration snapshot item.');
		return item;
	}

	#failure(
		code: DesktopAgentIntegrationManagementFailureCode,
		integrationId: DesktopAgentIntegrationId | null,
	): DesktopAgentIntegrationManagementOperationResult {
		return { code, integrationId, kind: 'failed', snapshot: this.getSnapshot() };
	}
}

function failureForAttempt(
	attempt: AgentIntegrationMutationAttempt,
): DesktopAgentIntegrationManagementFailureCode {
	switch (attempt.kind) {
		case 'conflict':
			return 'conflict';
		case 'manual-action-required':
			return 'manual-action-required';
		case 'attempted':
			return 'verification-failed';
		case 'failed':
			return 'operation-failed';
	}
}
