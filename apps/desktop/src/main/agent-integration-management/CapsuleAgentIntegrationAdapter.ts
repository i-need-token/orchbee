import type {
	DesktopAgentIntegrationId,
	DesktopAgentIntegrationManagementMode,
	DesktopAgentIntegrationSnapshotItem,
} from '@orchbee/desktop-client-protocol';

import type {
	AgentIntegrationAdapter,
	AgentIntegrationMutationAttempt,
} from './agent-integration-adapter';
import type { AgentObservationAdapterCapsule } from './file-agent-observation-adapter-capsule';

/** Ensures the stable payload exists only as part of an explicit enable. */
export class CapsuleAgentIntegrationAdapter implements AgentIntegrationAdapter {
	readonly #capsule: AgentObservationAdapterCapsule;
	readonly #delegate: AgentIntegrationAdapter;
	readonly integrationId: DesktopAgentIntegrationId;
	readonly managementMode: DesktopAgentIntegrationManagementMode;

	constructor(delegate: AgentIntegrationAdapter, capsule: AgentObservationAdapterCapsule) {
		this.#capsule = capsule;
		this.#delegate = delegate;
		this.integrationId = delegate.integrationId;
		this.managementMode = delegate.managementMode;
	}

	disable(): Promise<AgentIntegrationMutationAttempt> {
		return this.#delegate.disable();
	}

	async enable(): Promise<AgentIntegrationMutationAttempt> {
		if (this.managementMode === 'manual-only') return this.#delegate.enable();
		if (!(await this.#capsule.prepare())) return { kind: 'failed' };
		return this.#delegate.enable();
	}

	async inspect(): Promise<DesktopAgentIntegrationSnapshotItem> {
		const item = await this.#delegate.inspect();
		if (item.state !== 'enabled' || (await this.#capsule.isReady())) return item;
		return { ...item, state: 'update-required' };
	}
}
