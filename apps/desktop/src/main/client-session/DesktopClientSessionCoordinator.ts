import type {
	TerminalPresentationEvent,
	TerminalPresentationId,
} from '@orchbee/workbench/terminal-presentation';
import type {
	WorkbenchSharedDocument,
	WorkbenchSharedDocumentRevision,
} from '@orchbee/workbench/document';

import type { WorkbenchDocumentStore } from '../workbench-document';
import {
	decideDesktopClientDocumentProposal,
	type DesktopClientDocumentProposalDecision,
	type DesktopClientSessionPhase,
} from './decideDesktopClientDocumentProposal';

export type DesktopClientSessionPublication =
	| {
			readonly document: WorkbenchSharedDocument;
			readonly sequence: string;
			readonly type: 'shared-document-published';
	  }
	| {
			readonly event: TerminalPresentationEvent;
			readonly sequence: string;
			readonly type: 'terminal-presentation-event-published';
	  }
	| {
			readonly reason: 'invalid' | 'read-failed' | 'write-failed';
			readonly sequence: string;
			readonly type: 'client-session-coordinator-unavailable';
	  };

export type DesktopClientDocumentProposalResult =
	| {
			readonly document: WorkbenchSharedDocument;
			readonly kind: 'committed' | 'converged' | 'stale';
	  }
	| {
			readonly document: WorkbenchSharedDocument | null;
			readonly kind: 'rejected';
			readonly reason:
				| 'application-quiescing'
				| 'control-unavailable'
				| 'exit-transition'
				| 'revision-sequence';
	  }
	| {
			readonly document: WorkbenchSharedDocument | null;
			readonly kind: 'unavailable';
			readonly reason: 'invalid' | 'read-failed' | 'write-failed';
	  };

type DesktopClientSessionBootstrap =
	| {
			readonly defaultCwd: string;
			readonly document: WorkbenchSharedDocument;
			readonly initialPresentationId: TerminalPresentationId;
			readonly initializationDocument: WorkbenchSharedDocument;
			readonly kind: 'available';
			readonly sequence: string;
			readonly terminalHostName: string;
			readonly terminalEvents: readonly TerminalPresentationEvent[];
	  }
	| {
			readonly defaultCwd: string;
			readonly initialPresentationId: TerminalPresentationId;
			readonly initializationDocument: WorkbenchSharedDocument;
			readonly kind: 'missing';
			readonly sequence: string;
			readonly terminalHostName: string;
			readonly terminalEvents: readonly TerminalPresentationEvent[];
	  };

export interface DesktopClientSessionPublisher {
	disposeAttachments(): Promise<void>;
	publish(publication: DesktopClientSessionPublication): void;
}

export interface DesktopClientSessionHandle {
	readonly bootstrap: DesktopClientSessionBootstrap;
	readonly clientSessionId: string;
	dispose(): Promise<void>;
	propose(
		requestId: string,
		expectedRevision: WorkbenchSharedDocumentRevision | null,
		document: WorkbenchSharedDocument,
	): Promise<DesktopClientDocumentProposalResult>;
}

export interface DesktopClientSessionCoordinatorDependencies {
	readonly createClientSessionId: () => string;
	readonly defaultCwd: string;
	readonly getTerminalEvents: () => readonly TerminalPresentationEvent[];
	readonly initialPresentationId: TerminalPresentationId;
	readonly initializationDocument: WorkbenchSharedDocument;
	readonly reconcileDocument: (
		document: WorkbenchSharedDocument,
		publish: (event: TerminalPresentationEvent) => void,
	) => Promise<void>;
	readonly store: WorkbenchDocumentStore;
	readonly terminalHostName: string;
	readonly validateDocument: (document: WorkbenchSharedDocument) => boolean;
}

export type DesktopClientSessionRegistrationResult =
	| { readonly handle: DesktopClientSessionHandle; readonly kind: 'ready' }
	| {
			readonly kind: 'unavailable';
			readonly reason: 'control-unavailable' | 'invalid' | 'read-failed' | 'write-failed';
	  };

interface ClientSessionRecord {
	connected: boolean;
	readonly terminalExitEventKeys: Set<string>;
	retirementPromise: Promise<void> | undefined;
	readonly publisher: DesktopClientSessionPublisher;
	readonly requests: Map<
		string,
		{ readonly fingerprint: string; readonly result: Promise<DesktopClientDocumentProposalResult> }
	>;
}

export class DesktopClientSessionCoordinator {
	#canonicalDocument: WorkbenchSharedDocument | null = null;
	readonly #dependencies: DesktopClientSessionCoordinatorDependencies;
	#phase: DesktopClientSessionPhase = { kind: 'accepting' };
	#proposalTail = Promise.resolve();
	#sequence = 0n;
	readonly #sessions = new Map<string, ClientSessionRecord>();
	readonly #retiringSessions = new Map<string, ClientSessionRecord>();
	readonly #pendingTerminalExitPublications = new Map<
		string,
		Extract<
			DesktopClientSessionPublication,
			{ readonly type: 'terminal-presentation-event-published' }
		>
	>();
	#started = false;
	#unavailableReason: 'invalid' | 'read-failed' | 'write-failed' | undefined;
	#startResult:
		| { readonly kind: 'ready' }
		| { readonly kind: 'unavailable'; readonly reason: 'invalid' | 'read-failed' }
		| undefined;

	constructor(dependencies: DesktopClientSessionCoordinatorDependencies) {
		this.#dependencies = dependencies;
	}

	get canonicalDocument(): WorkbenchSharedDocument | null {
		return this.#canonicalDocument;
	}

	get sessionCount(): number {
		return this.#sessions.size;
	}

	async start(): Promise<
		| { readonly kind: 'ready' }
		| { readonly kind: 'unavailable'; readonly reason: 'invalid' | 'read-failed' }
	> {
		if (this.#startResult !== undefined) return this.#startResult;
		if (this.#started)
			throw new Error('Desktop client session coordinator start is already pending.');
		this.#started = true;
		const loaded = await this.#dependencies.store.load();
		if (loaded.kind === 'unavailable') {
			this.#phase = { kind: 'unavailable' };
			this.#unavailableReason = loaded.reason;
			this.#startResult = loaded;
			return loaded;
		}
		this.#canonicalDocument = loaded.kind === 'available' ? loaded.document : null;
		if (this.#canonicalDocument !== null) {
			if (!this.#dependencies.validateDocument(this.#canonicalDocument)) {
				this.#phase = { kind: 'unavailable' };
				this.#unavailableReason = 'invalid';
				this.#startResult = { kind: 'unavailable', reason: 'invalid' };
				return this.#startResult;
			}
			await this.#dependencies.reconcileDocument(
				this.#canonicalDocument,
				this.publishTerminalEvent,
			);
		}
		this.#startResult = { kind: 'ready' };
		return this.#startResult;
	}

	registerSession(
		publisher: DesktopClientSessionPublisher,
	): DesktopClientSessionRegistrationResult {
		if (
			this.#startResult?.kind !== 'ready' ||
			this.#phase.kind === 'disposed' ||
			this.#phase.kind === 'unavailable'
		) {
			return {
				kind: 'unavailable',
				reason:
					this.#startResult?.kind === 'unavailable'
						? this.#startResult.reason
						: (this.#unavailableReason ?? 'control-unavailable'),
			};
		}
		const clientSessionId = this.#allocateClientSessionId();
		const terminalEvents = this.#dependencies.getTerminalEvents();
		const record: ClientSessionRecord = {
			connected: true,
			terminalExitEventKeys: new Set(
				terminalEvents
					.filter((event) => event.type === 'terminal-exited')
					.map(terminalExitEventKey),
			),
			retirementPromise: undefined,
			publisher,
			requests: new Map(),
		};
		this.#sessions.set(clientSessionId, record);
		const bootstrap: DesktopClientSessionBootstrap =
			this.#canonicalDocument === null
				? {
						defaultCwd: this.#dependencies.defaultCwd,
						initialPresentationId: this.#dependencies.initialPresentationId,
						initializationDocument: this.#dependencies.initializationDocument,
						kind: 'missing',
						sequence: this.#sequence.toString(),
						terminalHostName: this.#dependencies.terminalHostName,
						terminalEvents,
					}
				: {
						defaultCwd: this.#dependencies.defaultCwd,
						document: this.#canonicalDocument,
						initialPresentationId: this.#dependencies.initialPresentationId,
						initializationDocument: this.#dependencies.initializationDocument,
						kind: 'available',
						sequence: this.#sequence.toString(),
						terminalHostName: this.#dependencies.terminalHostName,
						terminalEvents,
					};
		return {
			handle: {
				bootstrap,
				clientSessionId,
				dispose: () => this.#retireSession(clientSessionId),
				propose: (requestId, expectedRevision, document) =>
					this.#propose(clientSessionId, requestId, expectedRevision, document),
			},
			kind: 'ready',
		};
	}

	beginQuiescing(clientSessionId: string, mode: 'preserve' | 'stop'): boolean {
		if (!this.#sessions.has(clientSessionId)) return false;
		if (this.#phase.kind === 'quiescing') {
			if (this.#sessions.has(this.#phase.exitOwner) || this.#phase.mode !== mode) return false;
			this.#phase = { exitOwner: clientSessionId, kind: 'quiescing', mode };
			return true;
		}
		if (this.#phase.kind !== 'accepting') return false;
		this.#phase = { exitOwner: clientSessionId, kind: 'quiescing', mode };
		return true;
	}

	async dispose(): Promise<void> {
		if (this.#phase.kind !== 'disposed') this.#phase = { kind: 'disposed' };
		const sessionIds = new Set([...this.#sessions.keys(), ...this.#retiringSessions.keys()]);
		if (sessionIds.size === 0) return;
		const results = await Promise.allSettled(
			[...sessionIds].map(async (sessionId) => this.#retireSession(sessionId)),
		);
		const failure = results.find(
			(result): result is PromiseRejectedResult => result.status === 'rejected',
		);
		if (failure !== undefined) throw failure.reason;
	}

	readonly publishTerminalEvent = (event: TerminalPresentationEvent): boolean => {
		if (event.type !== 'terminal-exited') {
			return this.#broadcast({
				event,
				sequence: this.#nextSequence(),
				type: 'terminal-presentation-event-published',
			});
		}
		if (this.#terminalEventIsCaughtUp(event)) {
			this.#pendingTerminalExitPublications.delete(terminalExitEventKey(event));
			return true;
		}
		const key = terminalExitEventKey(event);
		const publication =
			this.#pendingTerminalExitPublications.get(key) ??
			({
				event,
				sequence: this.#nextSequence(),
				type: 'terminal-presentation-event-published',
			} as const);
		this.#pendingTerminalExitPublications.set(key, publication);
		const published = this.#broadcastTerminalExit(publication, key);
		if (published) this.#pendingTerminalExitPublications.delete(key);
		return published;
	};

	#allocateClientSessionId(): string {
		const clientSessionId = this.#dependencies.createClientSessionId();
		if (
			clientSessionId.length === 0 ||
			this.#sessions.has(clientSessionId) ||
			this.#retiringSessions.has(clientSessionId)
		) {
			throw new Error('Desktop client session identity allocation conflicted.');
		}
		return clientSessionId;
	}

	#propose(
		clientSessionId: string,
		requestId: string,
		expectedRevision: WorkbenchSharedDocumentRevision | null,
		document: WorkbenchSharedDocument,
	): Promise<DesktopClientDocumentProposalResult> {
		const session = this.#sessions.get(clientSessionId);
		if (session?.connected !== true) {
			return Promise.resolve({
				document: this.#canonicalDocument,
				kind: 'rejected',
				reason: 'control-unavailable',
			});
		}
		const fingerprint = JSON.stringify([expectedRevision, document]);
		const existing = session.requests.get(requestId);
		if (existing !== undefined) {
			if (existing.fingerprint === fingerprint) return existing.result;
			void this.#retireSession(clientSessionId).catch(() => undefined);
			return Promise.resolve({
				document: this.#canonicalDocument,
				kind: 'rejected',
				reason: 'control-unavailable',
			});
		}
		const result = this.#enqueueProposal(async () => {
			if (this.#sessions.get(clientSessionId)?.connected !== true) {
				return {
					document: this.#canonicalDocument,
					kind: 'rejected',
					reason: 'control-unavailable',
				} as const;
			}
			return this.#processProposal(clientSessionId, expectedRevision, document);
		});
		session.requests.set(requestId, { fingerprint, result });
		return result;
	}

	#enqueueProposal(
		operation: () => Promise<DesktopClientDocumentProposalResult>,
	): Promise<DesktopClientDocumentProposalResult> {
		const result = this.#proposalTail.then(operation);
		this.#proposalTail = result.then(
			() => undefined,
			() => undefined,
		);
		return result;
	}

	async #processProposal(
		clientSessionId: string,
		expectedRevision: WorkbenchSharedDocumentRevision | null,
		document: WorkbenchSharedDocument,
	): Promise<DesktopClientDocumentProposalResult> {
		if (!this.#dependencies.validateDocument(document)) {
			await this.#retireSession(clientSessionId);
			return {
				document: this.#canonicalDocument,
				kind: 'rejected',
				reason: 'control-unavailable',
			};
		}
		const decision = decideDesktopClientDocumentProposal(
			this.#canonicalDocument,
			{ clientSessionId, document, expectedRevision },
			this.#phase,
		);
		if (decision.kind !== 'commit') return resultFromDecision(decision, this.#canonicalDocument);
		const committed = await this.#dependencies.store.commit(expectedRevision, document);
		if (committed.kind !== 'committed') {
			if (committed.kind === 'unavailable') return this.#becomeUnavailable(committed.reason);
			return this.#becomeUnavailable('write-failed');
		}
		if (committed.revision !== document.revision) return this.#becomeUnavailable('write-failed');
		this.#canonicalDocument = document;
		this.#broadcast({
			document,
			sequence: this.#nextSequence(),
			type: 'shared-document-published',
		});
		await this.#dependencies.reconcileDocument(document, this.publishTerminalEvent);
		return { document, kind: 'committed' };
	}

	#becomeUnavailable(
		reason: 'invalid' | 'read-failed' | 'write-failed',
	): DesktopClientDocumentProposalResult {
		this.#phase = { kind: 'unavailable' };
		this.#unavailableReason = reason;
		this.#broadcast({
			reason,
			sequence: this.#nextSequence(),
			type: 'client-session-coordinator-unavailable',
		});
		return { document: this.#canonicalDocument, kind: 'unavailable', reason };
	}

	#broadcast(publication: DesktopClientSessionPublication): boolean {
		let published = true;
		const terminalEventKey =
			publication.type === 'terminal-presentation-event-published' &&
			publication.event.type === 'terminal-exited'
				? terminalExitEventKey(publication.event)
				: undefined;
		for (const [clientSessionId, session] of this.#sessions) {
			if (!session.connected) continue;
			try {
				session.publisher.publish(publication);
				if (terminalEventKey !== undefined) {
					session.terminalExitEventKeys.add(terminalEventKey);
				}
			} catch {
				published = false;
				void this.#retireSession(clientSessionId).catch(() => undefined);
			}
		}
		return published;
	}

	#broadcastTerminalExit(
		publication: Extract<
			DesktopClientSessionPublication,
			{ readonly type: 'terminal-presentation-event-published' }
		>,
		key: string,
	): boolean {
		let published = true;
		for (const [clientSessionId, session] of this.#sessions) {
			if (!session.connected || session.terminalExitEventKeys.has(key)) continue;
			try {
				session.publisher.publish(publication);
				session.terminalExitEventKeys.add(key);
			} catch {
				published = false;
				void this.#retireSession(clientSessionId).catch(() => undefined);
			}
		}
		return published;
	}

	#terminalEventIsCaughtUp(
		event: Extract<TerminalPresentationEvent, { readonly type: 'terminal-exited' }>,
	): boolean {
		const key = terminalExitEventKey(event);
		for (const session of this.#sessions.values()) {
			if (session.connected && !session.terminalExitEventKeys.has(key)) return false;
		}
		return true;
	}

	#nextSequence(): string {
		this.#sequence += 1n;
		return this.#sequence.toString();
	}

	async #retireSession(clientSessionId: string): Promise<void> {
		const session =
			this.#sessions.get(clientSessionId) ?? this.#retiringSessions.get(clientSessionId);
		if (session === undefined) return;
		if (session.retirementPromise !== undefined) return session.retirementPromise;
		if (session.connected) {
			session.connected = false;
			this.#sessions.delete(clientSessionId);
			this.#retiringSessions.set(clientSessionId, session);
		}
		let disposal: Promise<void>;
		try {
			disposal = session.publisher.disposeAttachments();
		} catch (error: unknown) {
			disposal = Promise.reject(error);
		}
		const operation = disposal.then(
			() => {
				this.#retiringSessions.delete(clientSessionId);
			},
			(error: unknown) => {
				session.retirementPromise = undefined;
				throw error;
			},
		);
		session.retirementPromise = operation;
		return operation;
	}
}

function terminalExitEventKey(
	event: Extract<TerminalPresentationEvent, { readonly type: 'terminal-exited' }>,
): string {
	return JSON.stringify(event);
}

function resultFromDecision(
	decision: Exclude<DesktopClientDocumentProposalDecision, { readonly kind: 'commit' }>,
	canonical: WorkbenchSharedDocument | null,
): DesktopClientDocumentProposalResult {
	if (decision.kind === 'converged' || decision.kind === 'stale') {
		return { document: decision.document, kind: decision.kind };
	}
	return { document: canonical, kind: 'rejected', reason: decision.reason };
}
