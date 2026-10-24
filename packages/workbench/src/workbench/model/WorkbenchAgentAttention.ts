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
	/** Display-only projection; canonical outcomes and notification inputs stay in observation. */
	readonly presentation: AgentRuntimeSnapshot;
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
	readonly #editedCompletions = new Map<AgentSessionId, string>();

	constructor(source: AgentSnapshotSource | undefined) {
		this.#source = source;
		const observation = source?.getSnapshot() ?? EMPTY_AGENT_RUNTIME_SNAPSHOT;
		this.#snapshot = {
			observation,
			presentation: observation,
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
		const completed = new Map(
			observation.sessions
				.filter((session) => projectAgentSession(session).status.kind === 'succeeded')
				.map((session) => [session.sessionId, completionVersion(session)]),
		);
		for (const [id, edited] of this.#editedCompletions) {
			if (completed.get(id) !== edited) this.#editedCompletions.delete(id);
		}
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

	/** The terminal adapter invokes this only after it accepts text, paste or an editing key. */
	terminalEdited = (presentationId: string): void => {
		this.#observe();
		let changed = false;
		for (const session of this.#snapshot.observation.sessions) {
			if (
				session.lifecycle !== 'live' ||
				projectAgentSession(session).status.kind !== 'succeeded' ||
				!session.presentationAttachments.some(
					(attachment) => attachment.presentationId === presentationId && attachment.navigable,
				)
			)
				continue;
			const completedVersion = completionVersion(session);
			if (this.#editedCompletions.get(session.sessionId) === completedVersion) continue;
			this.#editedCompletions.set(session.sessionId, completedVersion);
			changed = true;
		}
		if (changed) this.#publish(this.#snapshot);
	};

	#publish(snapshot: Pick<WorkbenchAgentAttentionSnapshot, 'observation' | 'unread'>): void {
		const observation = snapshot.observation;
		const presentation: AgentRuntimeSnapshot =
			this.#editedCompletions.size === 0
				? observation
				: {
						...observation,
						sessions: observation.sessions.map((session) =>
							this.#editedCompletions.get(session.sessionId) === completionVersion(session)
								? { ...session, lastOutcome: null, turn: { kind: 'idle' } }
								: session,
						),
					};
		this.#snapshot = { ...snapshot, presentation };
		for (const listener of this.#listeners) listener();
	}
}

function completionVersion(session: AgentSessionSnapshot): string {
	return JSON.stringify([
		session.observation.generation,
		session.lastOutcome,
		session.presentationAttachments.map((attachment) => [
			attachment.presentationId,
			attachment.generation,
		]),
	]);
}
