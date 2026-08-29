import type { DesktopAgentIntegrationId } from '@orchbee/desktop-client-protocol';

import {
	canonicalizeIntegrationIds,
	readAgentIntegrationReminderPreferences,
	writeAgentIntegrationReminderPreferences,
	type AgentIntegrationReminderPersistenceIssue,
	type AgentIntegrationReminderStorage,
} from './agent-integration-reminder-preferences-storage';

export interface AgentIntegrationReminderSnapshot {
	readonly activeIntegrationId: DesktopAgentIntegrationId | null;
	readonly dismissedIntegrationIds: readonly DesktopAgentIntegrationId[];
	readonly persistenceIssue: AgentIntegrationReminderPersistenceIssue | null;
	readonly revision: number;
}

export type AgentIntegrationReminderPreferenceUpdateResult =
	| { readonly kind: 'applied-and-saved' }
	| { readonly kind: 'applied-session-only' }
	| { readonly kind: 'disposed' }
	| { readonly kind: 'unchanged' };

type SnapshotListener = () => void;

export class AgentIntegrationReminderController {
	#consumedThisSession = false;
	#disposed = false;
	readonly #listeners = new Set<SnapshotListener>();
	#snapshot: AgentIntegrationReminderSnapshot;
	readonly #storage: AgentIntegrationReminderStorage;

	constructor(storage: AgentIntegrationReminderStorage) {
		this.#storage = storage;
		const restored = readAgentIntegrationReminderPreferences(storage);
		this.#snapshot = createSnapshot({
			activeIntegrationId: null,
			dismissedIntegrationIds: restored.preferences.dismissedIntegrationIds,
			persistenceIssue: restored.issue,
			revision: 0,
		});
	}

	getSnapshot = (): AgentIntegrationReminderSnapshot => this.#snapshot;

	subscribe = (listener: SnapshotListener): (() => void) => {
		if (this.#disposed) return () => undefined;
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	};

	offer(integrationId: DesktopAgentIntegrationId): boolean {
		if (
			this.#disposed ||
			this.#consumedThisSession ||
			this.#snapshot.dismissedIntegrationIds.includes(integrationId)
		) {
			return false;
		}
		this.#consumedThisSession = true;
		this.#replaceSnapshot({ activeIntegrationId: integrationId });
		return true;
	}

	hide(integrationId: DesktopAgentIntegrationId): void {
		if (this.#disposed || this.#snapshot.activeIntegrationId !== integrationId) return;
		this.#replaceSnapshot({ activeIntegrationId: null });
	}

	dismiss(
		integrationId: DesktopAgentIntegrationId,
	): AgentIntegrationReminderPreferenceUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		this.#consumedThisSession = true;
		const dismissedIntegrationIds = new Set(this.#snapshot.dismissedIntegrationIds);
		const changed = !dismissedIntegrationIds.has(integrationId);
		dismissedIntegrationIds.add(integrationId);
		if (!changed && this.#snapshot.activeIntegrationId !== integrationId) {
			return { kind: 'unchanged' };
		}
		return this.#commit(
			canonicalizeIntegrationIds(dismissedIntegrationIds),
			this.#snapshot.activeIntegrationId === integrationId
				? null
				: this.#snapshot.activeIntegrationId,
		);
	}

	restore(
		integrationId: DesktopAgentIntegrationId,
	): AgentIntegrationReminderPreferenceUpdateResult {
		if (this.#disposed) return { kind: 'disposed' };
		if (!this.#snapshot.dismissedIntegrationIds.includes(integrationId)) {
			return { kind: 'unchanged' };
		}
		return this.#commit(
			Object.freeze(
				this.#snapshot.dismissedIntegrationIds.filter((candidate) => candidate !== integrationId),
			),
			this.#snapshot.activeIntegrationId,
		);
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#listeners.clear();
	}

	#commit(
		dismissedIntegrationIds: readonly DesktopAgentIntegrationId[],
		activeIntegrationId: DesktopAgentIntegrationId | null,
	): AgentIntegrationReminderPreferenceUpdateResult {
		const saved = writeAgentIntegrationReminderPreferences(this.#storage, {
			dismissedIntegrationIds,
		});
		this.#replaceSnapshot({
			activeIntegrationId,
			dismissedIntegrationIds,
			persistenceIssue: saved ? null : 'write-failed',
		});
		return saved ? { kind: 'applied-and-saved' } : { kind: 'applied-session-only' };
	}

	#replaceSnapshot(patch: Partial<Omit<AgentIntegrationReminderSnapshot, 'revision'>>): void {
		this.#snapshot = createSnapshot({
			...this.#snapshot,
			...patch,
			revision: this.#snapshot.revision + 1,
		});
		for (const listener of this.#listeners) listener();
	}
}

function createSnapshot(
	snapshot: AgentIntegrationReminderSnapshot,
): AgentIntegrationReminderSnapshot {
	return Object.freeze({
		...snapshot,
		dismissedIntegrationIds: Object.freeze([...snapshot.dismissedIntegrationIds]),
	});
}
