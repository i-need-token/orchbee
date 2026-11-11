import type { WorkbenchAgentNotResumed, WorkbenchAgentStartFailure } from '../../../workbench';
import { parseWorkbenchResponse, type WorkbenchSnapshot } from '@orchbee/workbench-client-protocol';
import { readFile } from 'node:fs/promises';
import { describe, expect, it } from 'vitest';
import { z } from 'zod';

import { createServerAgentResumeNotices } from '../createServerAgentResumeNotices';
import { createServerAgentStartNotices } from '../createServerAgentStartNotices';

const REQUEST = '99999999999999999999999999999999';
const NOW = 1_800_000_000_000;

/** The corpus snapshots whose bound Presentation carries a one-shot fact, as the TUI's tests read them. */
const DOCUMENTS = z
	.object({
		documents: z.object({
			'snapshot-agent-not-resumed-r2': z.unknown(),
			'snapshot-agent-start-failure-r2': z.unknown(),
		}),
	})
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
	).documents;

function snapshotAt(document: keyof typeof DOCUMENTS, at: number): WorkbenchSnapshot {
	const snapshot: unknown = JSON.parse(
		JSON.stringify(DOCUMENTS[document]).replace('"at":9007199254740991', `"at":${String(at)}`),
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
	const resumeAt = (at: number) => snapshotAt('snapshot-agent-not-resumed-r2', at);

	it('says each fact once, and only while it is fresh', () => {
		let now = NOW;
		const session = new FakeSession(resumeAt(NOW - 1_000));
		const heard: WorkbenchAgentNotResumed[] = [];
		createServerAgentResumeNotices(session, () => now).subscribe((notice) => heard.push(notice));
		expect(heard).toEqual([{ providerId: 'claude-code', reason: 'prompt-unconfirmed' }]);
		// Every later snapshot still carries it; it is never said again.
		session.publish(resumeAt(NOW - 1_000));
		expect(heard).toHaveLength(1);
		// A new give-up on the same Presentation is a new fact.
		now = NOW + 5_000;
		session.publish(resumeAt(NOW + 5_000));
		expect(heard).toHaveLength(2);
		// A Client that first reads a fact long after it was published says nothing.
		const late: WorkbenchAgentNotResumed[] = [];
		createServerAgentResumeNotices(new FakeSession(resumeAt(NOW - 61_000)), () => NOW).subscribe(
			(notice) => late.push(notice),
		);
		expect(late).toEqual([]);
	});
});

describe('createServerAgentStartNotices', () => {
	const startAt = (at: number) => snapshotAt('snapshot-agent-start-failure-r2', at);

	it('says each fact once, only while it is fresh, and never one there before it listened', () => {
		let now = NOW;
		const session = new FakeSession(startAt(NOW - 1_000));
		const heard: WorkbenchAgentStartFailure[] = [];
		createServerAgentStartNotices(session, () => now).subscribe((notice) => heard.push(notice));
		// A fresh fact already present when the Client attached is not news.
		expect(heard).toEqual([]);
		now = NOW + 5_000;
		session.publish(startAt(NOW + 5_000));
		expect(heard).toEqual([{ providerId: 'codex', reason: 'not-found' }]);
		// Every later snapshot still carries it; it is never said again.
		session.publish(startAt(NOW + 5_000));
		expect(heard).toHaveLength(1);
		// A fact first read long after it was published says nothing.
		now = NOW + 70_000;
		session.publish(startAt(NOW + 6_000));
		expect(heard).toHaveLength(1);
	});
});
