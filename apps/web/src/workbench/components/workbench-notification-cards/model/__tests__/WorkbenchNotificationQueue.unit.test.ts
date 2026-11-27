import { afterEach, describe, expect, it, vi } from 'vitest';

import {
	WorkbenchNotificationQueue,
	type WorkbenchNotificationOutcome,
} from '../WorkbenchNotificationQueue';

afterEach(() => vi.useRealTimers());

function messages(queue: WorkbenchNotificationQueue): readonly string[] {
	return queue.getSnapshot().map((card) => card.message);
}

describe('WorkbenchNotificationQueue', () => {
	it('shows the newest three and closes a passing card ten seconds after it shows', async () => {
		vi.useFakeTimers();
		const queue = new WorkbenchNotificationQueue();
		const outcomes = new Map<string, WorkbenchNotificationOutcome>();
		const notify = (message: string, request: Parameters<typeof queue.notify>[0]): void => {
			void queue.notify(request).then((outcome) => outcomes.set(message, outcome));
		};
		notify('Fetched', { message: 'Fetched', severity: 'info' });
		notify('Push failed', { message: 'Push failed', severity: 'error' });
		notify('Parent repository', {
			buttons: ['Open', 'Never', 'Not now', 'More'],
			message: 'Parent repository',
			severity: 'warning',
		});
		notify('Too many changes', { message: 'Too many changes', severity: 'warning' });
		expect(messages(queue)).toEqual(['Push failed', 'Parent repository', 'Too many changes']);
		expect(queue.getSnapshot()[1]?.buttons).toEqual(['Open', 'Never', 'Not now']);

		// The waiting card's clock has not run: it gets its full ten seconds once it shows.
		vi.advanceTimersByTime(9_999);
		expect(messages(queue)).toContain('Too many changes');
		vi.advanceTimersByTime(1);
		expect(messages(queue)).toEqual(['Fetched', 'Push failed', 'Parent repository']);
		vi.advanceTimersByTime(9_999);
		expect(messages(queue)).toContain('Fetched');
		vi.advanceTimersByTime(1);
		vi.advanceTimersByTime(60_000);
		expect(messages(queue)).toEqual(['Push failed', 'Parent repository']);
		await Promise.resolve();
		expect(outcomes).toEqual(
			new Map([
				['Too many changes', { kind: 'dismissed' }],
				['Fetched', { kind: 'dismissed' }],
			]),
		);
	});

	it('settles each card once, with its button or a dismissal', async () => {
		vi.useFakeTimers();
		const queue = new WorkbenchNotificationQueue();
		const retry = queue.notify({ buttons: ['Retry'], message: 'Pull failed', severity: 'error' });
		const info = queue.notify({ message: 'Branch published', severity: 'info' });
		const [retryCard, infoCard] = queue.getSnapshot();
		if (retryCard === undefined || infoCard === undefined) throw new Error('Both cards show.');

		queue.choose(retryCard.id, 1);
		expect(messages(queue)).toEqual(['Pull failed', 'Branch published']);
		queue.choose(retryCard.id, 0);
		queue.dismiss(retryCard.id);
		await expect(retry).resolves.toEqual({ index: 0, kind: 'chosen' });

		queue.close();
		await expect(info).resolves.toEqual({ kind: 'dismissed' });
		expect(queue.getSnapshot()).toEqual([]);
		expect(vi.getTimerCount()).toBe(0);
	});
});
