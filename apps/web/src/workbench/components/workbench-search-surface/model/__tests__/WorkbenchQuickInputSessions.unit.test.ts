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

const CONTROL = Object.freeze({ altKey: false, ctrlKey: true, metaKey: false, shiftKey: false });
const CONTROL_SHIFT = Object.freeze({ ...CONTROL, shiftKey: true });

function released(key: string, held: { readonly ctrlKey?: boolean } = {}) {
	return { altKey: false, ctrlKey: held.ctrlKey === true, key, metaKey: false };
}

function activeId(sessions: WorkbenchQuickInputSessions): string | null | undefined {
	const session = sessions.getSnapshot();
	return session?.kind === 'pick' ? session.activeId : undefined;
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

	it('starts a pick on the item it names, on none without that item, and moves round', () => {
		const sessions = new WorkbenchQuickInputSessions();
		for (const [activation, entries, expected] of [
			['first', ITEMS, 'lf'],
			['second', ITEMS, 'crlf'],
			['last', ITEMS, 'crlf'],
			['second', ITEMS.slice(0, 1), null],
		] as const) {
			void sessions.pick({ activation, entries });
			expect(activeId(sessions)).toBe(expected);
		}
		const id = currentId(sessions);
		sessions.moveActive(id, 'previous');
		expect(activeId(sessions)).toBe('lf');

		void sessions.pick({ entries: ITEMS });
		expect(sessions.navigate('previous', [])).toBe(true);
		expect(activeId(sessions)).toBe('crlf');
		expect(sessions.navigate('next', [])).toBe(true);
		expect(activeId(sessions)).toBe('lf');
		sessions.changeFilter(currentId(sessions), 'cr');
		expect(activeId(sessions)).toBe('crlf');
		sessions.close();
		expect(sessions.navigate('next', [])).toBe(false);
	});

	it('accepts the active item once a modifier key that opened a quick navigate is let go', async () => {
		const sessions = new WorkbenchQuickInputSessions();
		const switched = sessions.pick({
			activation: 'second',
			entries: ITEMS,
			hideInput: true,
			quickNavigate: [CONTROL],
		});
		const id = currentId(sessions);
		expect(sessions.getSnapshot()).toMatchObject({ activeId: 'crlf', inputHidden: true });
		sessions.navigate('previous', [CONTROL_SHIFT]);
		// Shift let go while Control is held goes on moving, as VS Code's quick navigate does.
		sessions.keyUp(id, released('Shift', { ctrlKey: true }));
		sessions.keyUp(id, released('Tab', { ctrlKey: true }));
		expect(activeId(sessions)).toBe('lf');
		sessions.keyUp(id, released('Control'));
		await expect(switched).resolves.toEqual({ kind: 'accepted', value: '\n' });

		// Without an active item the pick stays open, and quick navigate has ended.
		const lone = sessions.pick({
			activation: 'second',
			entries: ITEMS.slice(0, 1),
			hideInput: true,
			quickNavigate: [CONTROL],
		});
		const loneId = currentId(sessions);
		sessions.keyUp(loneId, released('Control'));
		sessions.moveActive(loneId, 'next');
		sessions.keyUp(loneId, released('Control'));
		expect(sessions.getSnapshot()).toMatchObject({ activeId: 'lf', id: loneId });

		// A pick opened without held keys ignores them.
		const searched = sessions.pick({ entries: ITEMS });
		await expect(lone).resolves.toEqual({ kind: 'cancelled' });
		sessions.keyUp(currentId(sessions), released('Control'));
		expect(activeId(sessions)).toBe('lf');
		sessions.close();
		await expect(searched).resolves.toEqual({ kind: 'cancelled' });
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
