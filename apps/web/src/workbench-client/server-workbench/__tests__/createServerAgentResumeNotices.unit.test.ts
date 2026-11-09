import type { WorkbenchAgentNotResumed } from '../../../workbench';
import { parseWorkbenchResponse, type WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';
import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import { createServerAgentResumeNotices } from '../createServerAgentResumeNotices';

const REQUEST = '99999999999999999999999999999999';
const NOW = 1_800_000_000_000;

/** The corpus snapshot whose bound Presentation carries a resume fact, as the TUI's test reads it. */
const DOCUMENT = JSON.stringify(
	z
		.object({ documents: z.object({ 'snapshot-agent-not-resumed-r2': z.unknown() }) })
		.parse(
			JSON.parse(
				await readFile(
					new URL(
						'../../../../../../schemas/workbench/v1/workbench-conformance-v1.json',
						import.meta.url,
					),
					'utf8',
				),
			),
		).documents['snapshot-agent-not-resumed-r2'],
);

function snapshotAt(at: number): WorkbenchSnapshot {
	const snapshot: unknown = JSON.parse(
		DOCUMENT.replace('"at":9007199254740991', `"at":${String(at)}`),
	);
	const response = parseWorkbenchResponse({
		kind: 'workbench.snapshot.result',
		outcome: { kind: 'available', snapshot },
		protocolVersion: 1,
		requestId: REQUEST,
	});
	if (response.kind !== 'workbench.snapshot.result' || response.outcome.kind !== 'available') {
		throw new Error('The snapshot did not parse as available.');
	}
	return response.outcome.snapshot;
}

class FakeSession {
	readonly #listeners = new Set<(snapshot: WorkbenchSnapshot) => void>();
	snapshot: WorkbenchSnapshot;

	constructor(snapshot: WorkbenchSnapshot) {
		this.snapshot = snapshot;
	}

	publish(snapshot: WorkbenchSnapshot): void {
		this.snapshot = snapshot;
		for (const listener of this.#listeners) listener(snapshot);
	}

	subscribe(listener: (snapshot: WorkbenchSnapshot) => void): () => void {
		this.#listeners.add(listener);
		return () => this.#listeners.delete(listener);
	}
}

describe('createServerAgentResumeNotices', () => {
	it('says each fact once, and only while it is fresh', () => {
		let now = NOW;
		const session = new FakeSession(snapshotAt(NOW - 1_000));
		const heard: WorkbenchAgentNotResumed[] = [];
		createServerAgentResumeNotices(session, () => now).subscribe((notice) => heard.push(notice));
		expect(heard).toEqual([{ providerId: 'claude-code', reason: 'prompt-unconfirmed' }]);
		// Every later snapshot still carries it; it is never said again.
		session.publish(snapshotAt(NOW - 1_000));
		expect(heard).toHaveLength(1);
		// A new give-up on the same Presentation is a new fact.
		now = NOW + 5_000;
		session.publish(snapshotAt(NOW + 5_000));
		expect(heard).toHaveLength(2);
		// A Client that first reads a fact long after it was published says nothing.
		const late: WorkbenchAgentNotResumed[] = [];
		createServerAgentResumeNotices(new FakeSession(snapshotAt(NOW - 61_000)), () => NOW).subscribe(
			(notice) => late.push(notice),
		);
		expect(late).toEqual([]);
	});
});
