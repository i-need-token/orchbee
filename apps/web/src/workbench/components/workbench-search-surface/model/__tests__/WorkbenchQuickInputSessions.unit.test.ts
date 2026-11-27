import fc from 'fast-check';
import { describe, expect, it } from 'vitest';

import { WorkbenchQuickInputSessions } from '../WorkbenchQuickInputSessions';

interface Validation {
	readonly message: string;
	readonly severity: 'error' | 'info' | 'warning';
}

const ITEMS = Object.freeze([
	{ id: 'lf', kind: 'item', label: 'LF', value: '\n' },
	{ kind: 'separator' },
	{ id: 'crlf', kind: 'item', label: 'CRLF', value: '\r\n' },
] as const);

interface PendingCheck {
	readonly resolve: (validation: Validation | null) => void;
	readonly signal: AbortSignal;
}

function currentId(sessions: WorkbenchQuickInputSessions): number {
	const session = sessions.getSnapshot();
	if (session === null) throw new Error('No session is current.');
	return session.id;
}

function deferred<T>(): { readonly promise: Promise<T>; readonly resolve: (value: T) => void } {
	let settle: ((value: T) => void) | undefined;
	const promise = new Promise<T>((resolve) => {
		settle = resolve;
	});
	return { promise, resolve: (value) => settle?.(value) };
}

describe('WorkbenchQuickInputSessions', () => {
	it('cancels a replaced session, ignores its late events and accepts the current one', async () => {
		const sessions = new WorkbenchQuickInputSessions();
		const lineEnding = sessions.pick({ entries: ITEMS });
		const replacedId = currentId(sessions);
		const name = sessions.input({ prompt: 'Name', value: 'draft' });
		await expect(lineEnding).resolves.toEqual({ kind: 'cancelled' });

		const nameId = currentId(sessions);
		expect(sessions.accept(replacedId, 'crlf')).toBe(false);
		sessions.cancel(replacedId);
		expect(sessions.getSnapshot()).toMatchObject({ id: nameId, kind: 'input', value: 'draft' });
		sessions.changeValue(nameId, 'final');
		expect(sessions.accept(nameId)).toBe(true);
		await expect(name).resolves.toEqual({ kind: 'accepted', value: 'final' });

		const again = sessions.pick({ entries: ITEMS });
		expect(sessions.accept(currentId(sessions), 'missing')).toBe(false);
		expect(sessions.accept(currentId(sessions), 'crlf')).toBe(true);
		await expect(again).resolves.toEqual({ kind: 'accepted', value: '\r\n' });
		expect(sessions.getSnapshot()).toBeNull();
	});

	it('shows only the current value check and accepts once that check allows it', async () => {
		const sessions = new WorkbenchQuickInputSessions();
		const checks = new Map<string, PendingCheck>();
		const branch = sessions.input({
			validate: (value, signal) => {
				const answer = deferred<Validation | null>();
				checks.set(value, { resolve: answer.resolve, signal });
				return answer.promise;
			},
		});
		const id = currentId(sessions);
		sessions.changeValue(id, 'a');
		sessions.changeValue(id, 'a b');
		expect(checks.get('a')?.signal.aborted).toBe(true);
		expect(sessions.accept(id)).toBe(false);

		checks.get('a b')?.resolve({ message: 'No spaces', severity: 'error' });
		checks.get('a')?.resolve(null);
		await Promise.resolve();
		expect(sessions.getSnapshot()).toMatchObject({
			validating: false,
			validation: { message: 'No spaces', severity: 'error' },
		});
		expect(sessions.accept(id)).toBe(false);

		sessions.changeValue(id, 'ab');
		checks.get('ab')?.resolve({ message: 'Exists remotely', severity: 'warning' });
		await Promise.resolve();
		expect(sessions.accept(id)).toBe(true);
		await expect(branch).resolves.toEqual({ kind: 'accepted', value: 'ab' });

		let closedCheck: AbortSignal | undefined;
		const prompt = sessions.input({
			validate: (_value, signal) => {
				closedCheck = signal;
				return deferred<null>().promise;
			},
		});
		sessions.close();
		expect(closedCheck?.aborted).toBe(true);
		await expect(prompt).resolves.toEqual({ kind: 'cancelled' });
		expect(sessions.getSnapshot()).toBeNull();
	});

	it('settles every request but the current one, whatever the order of events', async () => {
		const operation = fc.oneof(
			fc.constant({ kind: 'pick' } as const),
			fc.constant({ kind: 'input' } as const),
			fc.record({ kind: fc.constant('accept' as const), stale: fc.boolean() }),
			fc.record({ kind: fc.constant('cancel' as const), stale: fc.boolean() }),
		);
		await fc.assert(
			fc.asyncProperty(fc.array(operation, { maxLength: 24 }), async (operations) => {
				const sessions = new WorkbenchQuickInputSessions();
				const outcomes: string[] = [];
				const settled: Promise<number>[] = [];
				let accepted = 0;
				for (const step of operations) {
					const stale = 'stale' in step && step.stale;
					const target = (sessions.getSnapshot()?.id ?? 0) - (stale ? 1 : 0);
					switch (step.kind) {
						case 'pick':
							settled.push(
								sessions.pick({ entries: ITEMS }).then((outcome) => outcomes.push(outcome.kind)),
							);
							break;
						case 'input':
							settled.push(
								sessions.input({ value: 'v' }).then((outcome) => outcomes.push(outcome.kind)),
							);
							break;
						case 'accept':
							if (sessions.accept(target, 'lf')) accepted += 1;
							break;
						case 'cancel':
							sessions.cancel(target);
							break;
					}
					await Promise.resolve();
				}
				await Promise.resolve();
				const open = sessions.getSnapshot() === null ? 0 : 1;
				expect(outcomes).toHaveLength(settled.length - open);
				expect(outcomes.filter((kind) => kind === 'accepted')).toHaveLength(accepted);
			}),
		);
	});
});
