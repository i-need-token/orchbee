import {
	createClientServerRequestId,
	decodeAgentReadResult,
	encodeAgentReadRequest,
	type AgentReadRequest,
	type AgentReadResult,
	type AgentSessionSummary,
	type ServerConnection,
} from '../server-connection';

/** 512 sessions at 100 a page, rounded up, as the TUI bounds its walk. */
const MAXIMUM_PAGES = 6;
const PAGE_LIMIT = 100;

type ListOutcome = Extract<
	AgentReadResult,
	{ readonly kind: 'agent.session.list.result' }
>['result']['outcome'];

type ServerAgentReadFailure = 'denied' | 'transport' | 'unavailable';

/**
 * One walk of the Server's Agent sessions. `complete` is false when the page bound or the second
 * stale cursor cut it short; what it found is still one revision's, never two spliced together.
 */
type ServerAgentSessionsRound =
	| {
			readonly complete: boolean;
			readonly items: readonly AgentSessionSummary[];
			readonly kind: 'listed';
	  }
	| { readonly kind: 'failed'; readonly reason: ServerAgentReadFailure };

type ServerAgentSession =
	| {
			readonly kind: 'found';
			readonly lifecycle: 'ended' | 'live';
			/** The terminals a person can be taken to, in the session's order. */
			readonly navigable: readonly {
				readonly presentationId: string;
				readonly visible: boolean;
			}[];
	  }
	| { readonly kind: 'not-found' }
	| { readonly kind: 'failed'; readonly reason: ServerAgentReadFailure };

/** Reads go over the window's connection, one at a time, each within what is left of `deadline`. */
interface ServerAgentRead {
	readonly connection: Pick<ServerConnection, 'exchange'>;
	/** Absolute, on the `performance.now()` clock. */
	readonly deadline: number;
}

/**
 * Reads every Agent session, ended ones included, as the TUI's board does: pages of one revision
 * under one deadline, walked again once when the Server reports the cursor stale or the revision
 * moves. A lost or late answer fails the round, since its effect on the walk is unknown.
 */
export async function listServerAgentSessions(
	read: ServerAgentRead,
): Promise<ServerAgentSessionsRound> {
	for (let walk = 0; ; walk += 1) {
		const items: AgentSessionSummary[] = [];
		let cursor: string | null = null;
		let revision: number | null = null;
		let restart = false;
		for (let page = 0; page < MAXIMUM_PAGES && !restart; page += 1) {
			const outcome: ListOutcome | null = await listPage(read, cursor);
			if (outcome === null) return { kind: 'failed', reason: 'transport' };
			if (outcome.kind === 'unavailable') return { kind: 'failed', reason: 'unavailable' };
			if (outcome.kind === 'rejected') {
				if (outcome.reason === 'read-denied') return { kind: 'failed', reason: 'denied' };
				restart = true;
			} else if (revision !== null && outcome.revision !== revision) {
				restart = true;
			} else {
				revision = outcome.revision;
				items.push(...outcome.items);
				if (outcome.nextCursor === null) return { complete: true, items, kind: 'listed' };
				cursor = outcome.nextCursor;
			}
		}
		if (!restart || walk > 0) return { complete: false, items, kind: 'listed' };
	}
}

/** Reads one session again, as a jump checks it before moving. */
export async function getServerAgentSession(
	read: ServerAgentRead,
	sessionId: string,
): Promise<ServerAgentSession> {
	const result = await exchange(read, {
		kind: 'agent.session.get.request',
		requestId: createClientServerRequestId(),
		sessionId,
	});
	if (result?.kind !== 'agent.session.get.result') return { kind: 'failed', reason: 'transport' };
	const { outcome } = result.result;
	switch (outcome.kind) {
		case 'found':
			return {
				kind: 'found',
				lifecycle: outcome.session.snapshot.lifecycle,
				navigable: outcome.session.snapshot.presentationAttachments
					.filter(({ navigable }) => navigable)
					.map(({ presentationId, visibility }) => ({
						presentationId,
						visible: visibility === 'visible',
					})),
			};
		case 'rejected':
			return outcome.reason === 'not-found'
				? { kind: 'not-found' }
				: { kind: 'failed', reason: 'denied' };
		case 'unavailable':
			return { kind: 'failed', reason: 'unavailable' };
	}
}

async function listPage(read: ServerAgentRead, cursor: string | null): Promise<ListOutcome | null> {
	const result = await exchange(read, {
		...(cursor === null ? {} : { cursor }),
		includeEnded: true,
		kind: 'agent.session.list.request',
		limit: PAGE_LIMIT,
		requestId: createClientServerRequestId(),
	});
	return result?.kind === 'agent.session.list.result' ? result.result.outcome : null;
}

/** One exchange on the root lane within what remains of the deadline; null when no matching answer came back. */
async function exchange(
	{ connection, deadline }: ServerAgentRead,
	request: AgentReadRequest,
): Promise<AgentReadResult | null> {
	const remaining = Math.floor(deadline - performance.now());
	if (remaining <= 0) return null;
	try {
		const result = decodeAgentReadResult(
			await connection.exchange(encodeAgentReadRequest(request), {
				timeoutMilliseconds: remaining,
			}),
		);
		return result.requestId === request.requestId ? result : null;
	} catch {
		return null;
	}
}
