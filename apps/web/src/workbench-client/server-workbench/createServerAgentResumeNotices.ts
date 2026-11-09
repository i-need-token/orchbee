import type { WorkbenchAgentNotResumed, WorkbenchAgentResumeNotices } from '../../workbench';
import type {
	WorkbenchPanelNode,
	WorkbenchPresentation,
	WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

import type { ServerWorkbenchSession } from './ServerWorkbenchSession';

type AgentResumeSession = Pick<ServerWorkbenchSession, 'snapshot' | 'subscribe'>;
type AgentNotResumedFact = NonNullable<WorkbenchPresentation['agentNotResumed']>;

/** How far a fact's time may lie from this Client's clock and still read as news, as in the TUI. */
const FRESH_MILLISECONDS = 60_000;

/**
 * The Server's one-shot facts that a resume after a restart typed nothing, as the TUI shows them:
 * each fact once, only while it is fresh, newest first. A Client that connects later, or reads a
 * snapshot the fact has long sat in, hears nothing of it.
 */
export function createServerAgentResumeNotices(
	session: AgentResumeSession,
	now: () => number = Date.now,
): WorkbenchAgentResumeNotices {
	return {
		subscribe: (listener) => {
			let seen = new Set<string>();
			const observe = (snapshot: WorkbenchSnapshot): void => {
				const present = new Set<string>();
				let newest: AgentNotResumedFact | null = null;
				const time = now();
				for (const { fact, presentationId } of facts(snapshot)) {
					const key = `${presentationId}:${fact.at}`;
					if (
						!seen.has(key) &&
						Math.abs(time - fact.at) <= FRESH_MILLISECONDS &&
						(newest === null || fact.at > newest.at)
					) {
						newest = fact;
					}
					present.add(key);
				}
				// A fact never comes back with the same time, so one that left is forgotten.
				seen = present;
				if (newest !== null) listener(notice(newest));
			};
			observe(session.snapshot);
			return session.subscribe(observe);
		},
	};
}

function notice(fact: AgentNotResumedFact): WorkbenchAgentNotResumed {
	return { providerId: fact.providerId, reason: fact.reason };
}

function facts(
	snapshot: WorkbenchSnapshot,
): readonly { readonly fact: AgentNotResumedFact; readonly presentationId: string }[] {
	const found: { readonly fact: AgentNotResumedFact; readonly presentationId: string }[] = [];
	const walk = (node: WorkbenchPanelNode): void => {
		if (node.kind === 'panel') {
			for (const presentation of [node.primaryPresentation, ...node.childPresentations]) {
				if (presentation.agentNotResumed !== undefined) {
					found.push({
						fact: presentation.agentNotResumed,
						presentationId: presentation.presentationId,
					});
				}
			}
			return;
		}
		for (const slot of node.children) walk(slot.node);
	};
	for (const context of snapshot.terminalContexts) {
		for (const tab of context.tabs) walk(tab.root);
	}
	return found;
}
