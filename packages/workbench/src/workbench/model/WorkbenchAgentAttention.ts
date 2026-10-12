import {
	projectAgentSession,
	type AgentRuntimeSnapshot,
	type AgentSessionId,
	type AgentSessionSnapshot,
	type AgentSnapshotSource,
} from '@orchbee/agent';

import { EMPTY_AGENT_RUNTIME_SNAPSHOT } from '../constants/empty-agent-runtime-snapshot';

export interface WorkbenchAgentAttentionSnapshot {
	readonly observation: AgentRuntimeSnapshot;
	readonly unread: ReadonlySet<AgentSessionId>;
}

// Same meaningful version as the native Client: status, lifecycle, generation and
// freshness. Presentation visibility and provider metadata cannot reopen a read version.
function version(session: AgentSessionSnapshot): string {
	return JSON.stringify([
		projectAgentSession(session).status,
		session.lifecycle,
		session.observation.generation,
		session.observation.freshness,
	]);
}

/** Client-local acknowledgement; never writes the canonical Agent runtime. */
export class WorkbenchAgentAttention {
	readonly #source: AgentSnapshotSource | undefined;
	readonly #listeners = new Set<() => void>();
	#unsubscribe: (() => void) | undefined;
	#snapshot: WorkbenchAgentAttentionSnapshot;

	constructor(source: AgentSnapshotSource | undefined) {
		this.#source = source;
		this.#snapshot = {
			observation: source?.getSnapshot() ?? EMPTY_AGENT_RUNTIME_SNAPSHOT,
			unread: new Set(),
		};
	}

	getSnapshot = (): WorkbenchAgentAttentionSnapshot => this.#snapshot;

	subscribe = (listener: () => void): (() => void) => {
		this.#listeners.add(listener);
		if (this.#listeners.size === 1) {
			this.#unsubscribe = this.#source?.subscribe(this.#observe);
			this.#observe();
		}
		return () => {
			this.#listeners.delete(listener);
			if (this.#listeners.size === 0) {
				this.#unsubscribe?.();
				this.#unsubscribe = undefined;
			}
		};
	};

	#observe = (): void => {
		const observation = this.#source?.getSnapshot() ?? EMPTY_AGENT_RUNTIME_SNAPSHOT;
		if (observation === this.#snapshot.observation) return;
		const previous = new Map(
			this.#snapshot.observation.sessions.map((session) => [session.sessionId, version(session)]),
		);
		const unread = new Set<AgentSessionId>();
		for (const session of observation.sessions) {
			if (
				this.#snapshot.unread.has(session.sessionId) ||
				previous.get(session.sessionId) !== version(session)
			) {
				unread.add(session.sessionId);
			}
		}
		this.#publish({ observation, unread });
	};

	/** A stale paint, hidden attachment or unfocused window cannot acknowledge new work. */
	acknowledge(
		painted: WorkbenchAgentAttentionSnapshot,
		visiblePresentations: ReadonlySet<string>,
		focused: boolean,
	): void {
		if (
			!focused ||
			painted !== this.#snapshot ||
			this.#source?.getSnapshot() !== painted.observation
		)
			return;
		const unread = new Set(painted.unread);
		for (const session of painted.observation.sessions) {
			if (
				session.lifecycle === 'live' &&
				session.presentationAttachments.some(
					(attachment) =>
						attachment.navigable &&
						attachment.visibility !== 'hidden' &&
						visiblePresentations.has(attachment.presentationId),
				)
			)
				unread.delete(session.sessionId);
		}
		if (unread.size !== painted.unread.size) this.#publish({ ...painted, unread });
	}

	#publish(snapshot: WorkbenchAgentAttentionSnapshot): void {
		this.#snapshot = snapshot;
		for (const listener of this.#listeners) listener();
	}
}
