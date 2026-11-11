import type {
	WorkbenchPanelNode,
	WorkbenchPresentation,
	WorkbenchSnapshot,
} from '@orchbee/workbench-client-protocol';

import type { ServerWorkbenchSession } from './ServerWorkbenchSession';

export type ServerPresentationFactSession = Pick<ServerWorkbenchSession, 'snapshot' | 'subscribe'>;

/** How far a fact's time may lie from this Client's clock and still read as news, as in the TUI. */
const FRESH_MILLISECONDS = 60_000;

interface ServerPresentationFactRule {
	readonly now: () => number;
	/** A fact found with a time before this Unix millisecond is old, however fresh. */
	readonly since: number;
}

/**
 * The rule the Server's one-shot Presentation facts are shown by, as the TUI's: each fact once,
 * keyed by its Presentation and time, only while it is news, newest first. A fact is news while it
 * is fresh by this Client's clock and not older than `since`; a later snapshot that still carries it
 * never shows it again.
 */
export function subscribeServerPresentationFacts<Fact extends { readonly at: number }>(
	session: ServerPresentationFactSession,
	read: (presentation: WorkbenchPresentation) => Fact | undefined,
	listener: (fact: Fact) => void,
	{ now, since }: ServerPresentationFactRule,
): () => void {
	let seen = new Set<string>();
	const observe = (snapshot: WorkbenchSnapshot): void => {
		const present = new Set<string>();
		let newest: Fact | null = null;
		const time = now();
		for (const presentation of presentations(snapshot)) {
			const fact = read(presentation);
			if (fact === undefined) continue;
			const key = `${presentation.presentationId}:${fact.at}`;
			if (
				!seen.has(key) &&
				fact.at >= since &&
				Math.abs(time - fact.at) <= FRESH_MILLISECONDS &&
				(newest === null || fact.at > newest.at)
			) {
				newest = fact;
			}
			present.add(key);
		}
		// A fact never comes back with the same time, so one that left is forgotten.
		seen = present;
		if (newest !== null) listener(newest);
	};
	observe(session.snapshot);
	return session.subscribe(observe);
}

function presentations(snapshot: WorkbenchSnapshot): readonly WorkbenchPresentation[] {
	const found: WorkbenchPresentation[] = [];
	const walk = (node: WorkbenchPanelNode): void => {
		if (node.kind === 'panel') {
			found.push(node.primaryPresentation, ...node.childPresentations);
			return;
		}
		for (const slot of node.children) walk(slot.node);
	};
	for (const context of snapshot.terminalContexts) {
		for (const tab of context.tabs) walk(tab.root);
	}
	return found;
}
