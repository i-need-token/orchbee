import type { ActionDisposable } from '@orchbee/actions';
import { describe, expect, it, vi } from 'vitest';

import type {
	WorkbenchSearchProvider,
	WorkbenchSearchProviderResult,
	WorkbenchSearchProviderSection,
} from '../../../../types/workbench-search-provider';
import { WorkbenchSearchProviderCoordinator } from '../WorkbenchSearchProviderCoordinator';

interface Deferred<T> {
	readonly promise: Promise<T>;
	reject(reason?: unknown): void;
	resolve(value: T): void;
}

function noop(): void {}

function deferred<T>(): Deferred<T> {
	let rejectPromise: (reason?: unknown) => void = noop;
	let resolvePromise: (value: T) => void = noop;
	const promise = new Promise<T>((resolve, reject) => {
		rejectPromise = reject;
		resolvePromise = resolve;
	});
	return { promise, reject: rejectPromise, resolve: resolvePromise };
}

function providerResult(
	id: string,
	overrides: Partial<WorkbenchSearchProviderResult> = {},
): WorkbenchSearchProviderResult {
	return {
		availability: { kind: 'enabled' },
		canonicalLabel: id,
		icon: 'action',
		id,
		invocation: {
			kind: 'requires-input',
			unbound: {
				actionId: `action.${id}`,
				inputName: 'input',
				invocation: {
					scope: { id: 'workbench', parent: null },
					source: 'command-surface',
				},
			},
		},
		keywords: [],
		label: id,
		...overrides,
	};
}

function providerSection(
	id: string,
	results: readonly WorkbenchSearchProviderResult[] = [providerResult(id)],
): WorkbenchSearchProviderSection {
	return { id, label: id, order: 10, results };
}

function createProvider(
	id: string,
	overrides: Partial<WorkbenchSearchProvider> = {},
): WorkbenchSearchProvider {
	return {
		filters: ['all', 'agents'],
		getDefaultSections: () => [providerSection(`${id}-default`)],
		id,
		label: id,
		priority: 20,
		resultLimit: 5,
		search: (query) => [providerSection(`${id}-${query}`)],
		...overrides,
	};
}

async function flushOperations(): Promise<void> {
	await Promise.resolve();
	await Promise.resolve();
	await Promise.resolve();
}

describe('WorkbenchSearchProviderCoordinator', () => {
	it('rejects ambiguous provider registration contracts', () => {
		expect(
			() =>
				new WorkbenchSearchProviderCoordinator(
					[createProvider('agents'), createProvider('agents')],
					'en-US',
				),
		).toThrow(/registered twice/u);
		expect(
			() =>
				new WorkbenchSearchProviderCoordinator([createProvider('', { label: 'Agents' })], 'en-US'),
		).toThrow(/id must not be empty/u);
		expect(
			() =>
				new WorkbenchSearchProviderCoordinator([createProvider('agents', { label: '' })], 'en-US'),
		).toThrow(/label must not be empty/u);
		expect(
			() =>
				new WorkbenchSearchProviderCoordinator(
					[createProvider('agents', { resultLimit: 0 })],
					'en-US',
				),
		).toThrow(/positive integer resultLimit/u);
	});

	it('selects providers by filter and chooses default or query operations', async () => {
		const getDefaultSections = vi.fn<WorkbenchSearchProvider['getDefaultSections']>(() => [
			providerSection('default'),
		]);
		const search = vi.fn<WorkbenchSearchProvider['search']>(() => [providerSection('query')]);
		const provider = createProvider('agents', { getDefaultSections, search });
		const coordinator = new WorkbenchSearchProviderCoordinator([provider], 'en-US');

		coordinator.request({
			filter: 'settings',
			query: '',
			queryRevision: 0,
			sessionId: 1,
		});
		await flushOperations();
		expect(getDefaultSections).not.toHaveBeenCalled();
		expect(search).not.toHaveBeenCalled();

		coordinator.request({ filter: 'all', query: '   ', queryRevision: 1, sessionId: 1 });
		await flushOperations();
		expect(getDefaultSections).toHaveBeenCalledOnce();

		coordinator.request({ filter: 'all', query: 'shell', queryRevision: 2, sessionId: 1 });
		await flushOperations();
		expect(search).toHaveBeenCalledWith(
			'shell',
			{ filter: 'all', locale: 'en-US' },
			expect.any(AbortSignal),
		);
	});

	it('aborts replaced work and rejects a provider that completes an old request late', async () => {
		const first = deferred<readonly WorkbenchSearchProviderSection[]>();
		const second = deferred<readonly WorkbenchSearchProviderSection[]>();
		const signals: AbortSignal[] = [];
		const search = vi.fn<WorkbenchSearchProvider['search']>(
			(_query: string, _context: unknown, signal: AbortSignal) => {
				signals.push(signal);
				return signals.length === 1 ? first.promise : second.promise;
			},
		);
		const coordinator = new WorkbenchSearchProviderCoordinator(
			[createProvider('agents', { search })],
			'en-US',
		);

		coordinator.request({ filter: 'all', query: 'old', queryRevision: 1, sessionId: 1 });
		await flushOperations();
		coordinator.request({ filter: 'all', query: 'new', queryRevision: 2, sessionId: 1 });
		await flushOperations();
		expect(signals[0]?.aborted).toBe(true);

		first.resolve([providerSection('stale')]);
		await flushOperations();
		expect(coordinator.getSnapshot().runs[0]).toMatchObject({
			phase: 'loading',
			queryRevision: 2,
		});

		second.resolve([providerSection('current')]);
		await flushOperations();
		expect(coordinator.getSnapshot().runs[0]).toMatchObject({
			phase: 'ready',
			queryRevision: 2,
			sections: [{ id: 'agents:current' }],
		});
	});

	it('keeps a request submitted during cancellation ahead of the superseded request', async () => {
		const old = deferred<readonly WorkbenchSearchProviderSection[]>();
		const search = vi.fn<WorkbenchSearchProvider['search']>((query, _context, signal) => {
			if (query === 'old') {
				signal.addEventListener('abort', () => {
					coordinator.request({ filter: 'all', query: 'latest', queryRevision: 3, sessionId: 1 });
				});
				return old.promise;
			}
			return [providerSection(query)];
		});
		const coordinator = new WorkbenchSearchProviderCoordinator(
			[createProvider('agents', { search })],
			'en-US',
		);
		try {
			coordinator.request({ filter: 'all', query: 'old', queryRevision: 1, sessionId: 1 });
			await flushOperations();
			coordinator.request({ filter: 'all', query: 'superseded', queryRevision: 2, sessionId: 1 });
			await flushOperations();
			expect(search.mock.calls.map(([query]) => query)).toEqual(['old', 'latest']);
			expect(coordinator.getSnapshot()).toMatchObject({
				request: { query: 'latest', queryRevision: 3 },
				runs: [{ phase: 'ready', sections: [{ id: 'agents:latest' }] }],
			});
		} finally {
			coordinator.dispose();
			old.resolve([]);
		}
	});

	it('starts the latest run only once when a snapshot listener replaces the query', async () => {
		const search = vi.fn<WorkbenchSearchProvider['search']>((query) => [providerSection(query)]);
		const coordinator = new WorkbenchSearchProviderCoordinator(
			[createProvider('agents', { search })],
			'en-US',
		);
		coordinator.subscribe(() => {
			if (coordinator.getSnapshot().request?.query === 'superseded') {
				coordinator.request({ filter: 'all', query: 'latest', queryRevision: 2, sessionId: 1 });
			}
		});
		try {
			coordinator.request({ filter: 'all', query: 'superseded', queryRevision: 1, sessionId: 1 });
			await flushOperations();
			expect(search.mock.calls.map(([query]) => query)).toEqual(['latest']);
			expect(coordinator.getSnapshot().runs).toMatchObject([
				{ phase: 'ready', sections: [{ id: 'agents:latest' }] },
			]);
		} finally {
			coordinator.dispose();
		}
	});

	it('keeps the newest model change when cancellation synchronously announces another change', async () => {
		let notifyModelChanged: () => void = noop;
		const old = deferred<readonly WorkbenchSearchProviderSection[]>();
		const search = vi
			.fn<WorkbenchSearchProvider['search']>()
			.mockImplementationOnce((_query, _context, signal) => {
				signal.addEventListener('abort', () => notifyModelChanged());
				return old.promise;
			})
			.mockImplementation(() => [providerSection('latest')]);
		const coordinator = new WorkbenchSearchProviderCoordinator(
			[
				createProvider('agents', {
					search,
					subscribe: (listener) => {
						notifyModelChanged = listener;
						return { dispose: noop };
					},
				}),
			],
			'en-US',
		);
		const connection = coordinator.connect();
		try {
			coordinator.request({ filter: 'all', query: 'run', queryRevision: 1, sessionId: 1 });
			await flushOperations();
			notifyModelChanged();
			await flushOperations();
			expect(search).toHaveBeenCalledTimes(2);
			expect(coordinator.getSnapshot().runs).toMatchObject([
				{ attempt: 2, phase: 'ready', sections: [{ id: 'agents:latest' }] },
			]);
		} finally {
			connection.dispose();
			old.resolve([]);
		}
	});

	it('does not invoke queued provider work after the request is closed', async () => {
		const search = vi.fn<WorkbenchSearchProvider['search']>(() => []);
		const coordinator = new WorkbenchSearchProviderCoordinator(
			[createProvider('agents', { search })],
			'en-US',
		);
		coordinator.request({ filter: 'all', query: 'queued', queryRevision: 1, sessionId: 1 });
		coordinator.close();
		await flushOperations();
		expect(search).not.toHaveBeenCalled();
		expect(coordinator.getSnapshot()).toEqual({ request: null, runs: [] });
		coordinator.dispose();
	});

	it('isolates failure, retries only that provider, and preserves a ready peer', async () => {
		const retry = deferred<readonly WorkbenchSearchProviderSection[]>();
		let agentsAttempt = 0;
		const agents = createProvider('agents', {
			search: () => {
				agentsAttempt += 1;
				return agentsAttempt === 1 ? Promise.reject(new Error('offline')) : retry.promise;
			},
		});
		const commands = createProvider('commands', {
			filters: ['all'],
			search: () => [providerSection('commands-ready')],
		});
		const coordinator = new WorkbenchSearchProviderCoordinator([agents, commands], 'en-US');

		coordinator.request({ filter: 'all', query: 'run', queryRevision: 1, sessionId: 1 });
		await flushOperations();
		expect(coordinator.getSnapshot().runs).toMatchObject([
			{ phase: 'failed', providerId: 'agents' },
			{ phase: 'ready', providerId: 'commands' },
		]);

		coordinator.retry('agents');
		expect(coordinator.getSnapshot().runs).toMatchObject([
			{ attempt: 1, phase: 'loading', providerId: 'agents' },
			{ attempt: 0, phase: 'ready', providerId: 'commands' },
		]);
		retry.resolve([providerSection('agents-ready')]);
		await flushOperations();
		expect(coordinator.getSnapshot().runs).toMatchObject([
			{ attempt: 1, phase: 'ready', providerId: 'agents' },
			{ attempt: 0, phase: 'ready', providerId: 'commands' },
		]);
	});

	it('restarts only the provider whose owned model changed', async () => {
		let notifyModelChanged: () => void = noop;
		const dispose = vi.fn<() => void>();
		const subscribe = (listener: () => void): ActionDisposable => {
			notifyModelChanged = listener;
			return { dispose };
		};
		const search = vi.fn<WorkbenchSearchProvider['search']>(() => [providerSection('agents')]);
		const coordinator = new WorkbenchSearchProviderCoordinator(
			[createProvider('agents', { search, subscribe }), createProvider('commands')],
			'en-US',
		);
		coordinator.connect();
		coordinator.request({ filter: 'all', query: 'run', queryRevision: 1, sessionId: 1 });
		await flushOperations();

		notifyModelChanged();
		await flushOperations();
		expect(search).toHaveBeenCalledTimes(2);
		expect(coordinator.getSnapshot().runs).toMatchObject([
			{ attempt: 1, phase: 'ready', providerId: 'agents' },
			{ attempt: 0, phase: 'ready', providerId: 'commands' },
		]);

		coordinator.dispose();
		expect(dispose).toHaveBeenCalledOnce();
	});

	it('releases earlier subscriptions when a later provider cannot connect', () => {
		const failure = new Error('subscription unavailable');
		const releaseFirst = vi.fn<() => void>();
		const releaseSecond = vi.fn<() => void>();
		const subscribeSecond = vi
			.fn<NonNullable<WorkbenchSearchProvider['subscribe']>>()
			.mockImplementationOnce(() => {
				throw failure;
			})
			.mockImplementation(() => ({ dispose: releaseSecond }));
		const coordinator = new WorkbenchSearchProviderCoordinator(
			[
				createProvider('first', { subscribe: () => ({ dispose: releaseFirst }) }),
				createProvider('second', { subscribe: subscribeSecond }),
			],
			'en-US',
		);
		expect(() => coordinator.connect()).toThrow(failure);
		expect(releaseFirst).toHaveBeenCalledOnce();
		const recovered = coordinator.connect();
		recovered.dispose();
		expect(releaseFirst).toHaveBeenCalledTimes(2);
		expect(releaseSecond).toHaveBeenCalledOnce();
	});

	it('releases every subscription once even when a disposer throws', () => {
		const failure = new Error('subscription cleanup failed');
		const releaseFirst = vi.fn<() => void>(() => {
			throw failure;
		});
		const releaseSecond = vi.fn<() => void>();
		const coordinator = new WorkbenchSearchProviderCoordinator(
			[
				createProvider('first', { subscribe: () => ({ dispose: releaseFirst }) }),
				createProvider('second', { subscribe: () => ({ dispose: releaseSecond }) }),
			],
			'en-US',
		);
		const connection = coordinator.connect();
		expect(() => connection.dispose()).toThrow(failure);
		expect(releaseFirst).toHaveBeenCalledOnce();
		expect(releaseSecond).toHaveBeenCalledOnce();
		expect(() => connection.dispose()).not.toThrow();
		expect(releaseFirst).toHaveBeenCalledOnce();
		expect(releaseSecond).toHaveBeenCalledOnce();
	});

	it('retains both acquisition and rollback failures after releasing owned subscriptions', () => {
		const acquisitionFailure = new Error('subscription unavailable');
		const cleanupFailure = new Error('subscription cleanup failed');
		const release = vi.fn<() => void>(() => {
			throw cleanupFailure;
		});
		const coordinator = new WorkbenchSearchProviderCoordinator(
			[
				createProvider('first', { subscribe: () => ({ dispose: release }) }),
				createProvider('second', {
					subscribe: () => {
						throw acquisitionFailure;
					},
				}),
			],
			'en-US',
		);
		let failure: unknown;
		try {
			coordinator.connect();
		} catch (error) {
			failure = error;
		}
		expect(failure).toBeInstanceOf(AggregateError);
		if (!(failure instanceof AggregateError)) throw new Error('Expected both connection failures');
		expect(failure.errors).toEqual([acquisitionFailure, cleanupFailure]);
		expect(release).toHaveBeenCalledOnce();
		coordinator.dispose();
		expect(release).toHaveBeenCalledOnce();
	});

	it('releases a subscription returned after disposal during acquisition', () => {
		const release = vi.fn<() => void>();
		const subscribeLater = vi.fn<NonNullable<WorkbenchSearchProvider['subscribe']>>();
		const coordinator = new WorkbenchSearchProviderCoordinator(
			[
				createProvider('first', {
					subscribe: () => {
						coordinator.dispose();
						return { dispose: release };
					},
				}),
				createProvider('later', { subscribe: subscribeLater }),
			],
			'en-US',
		);
		const connection = coordinator.connect();
		expect(release).toHaveBeenCalledOnce();
		expect(subscribeLater).not.toHaveBeenCalled();
		connection.dispose();
		coordinator.dispose();
		expect(release).toHaveBeenCalledOnce();
		expect(() => coordinator.connect()).toThrow('disposed');
	});

	it('keeps a new connection alive when an older connection is disposed again', () => {
		const releasePrevious = vi.fn<() => void>();
		const releaseCurrent = vi.fn<() => void>();
		const subscribe = vi
			.fn<NonNullable<WorkbenchSearchProvider['subscribe']>>()
			.mockReturnValueOnce({ dispose: releasePrevious })
			.mockReturnValueOnce({ dispose: releaseCurrent });
		const coordinator = new WorkbenchSearchProviderCoordinator(
			[createProvider('agents', { subscribe })],
			'en-US',
		);
		const previous = coordinator.connect();
		previous.dispose();
		const current = coordinator.connect();
		try {
			previous.dispose();
			expect(releasePrevious).toHaveBeenCalledOnce();
			expect(releaseCurrent).not.toHaveBeenCalled();
		} finally {
			current.dispose();
		}
		expect(releaseCurrent).toHaveBeenCalledOnce();
	});

	it('normalizes identities, removes hidden and duplicate results, and enforces the provider limit', async () => {
		const coordinator = new WorkbenchSearchProviderCoordinator(
			[
				createProvider('agents', {
					resultLimit: 2,
					search: () => [
						providerSection('people', [
							providerResult('alpha', { recommended: true }),
							providerResult('hidden', { availability: { kind: 'hidden' } }),
							providerResult('alpha'),
						]),
						providerSection('people', [
							providerResult('beta', { visibility: 'when-no-results' }),
							providerResult('gamma'),
						]),
					],
				}),
			],
			'en-US',
		);
		coordinator.request({ filter: 'all', query: 'a', queryRevision: 1, sessionId: 1 });
		await flushOperations();

		const run = coordinator.getSnapshot().runs[0];
		expect(run?.phase).toBe('ready');
		if (run?.phase !== 'ready') {
			throw new Error('Expected the provider to be ready');
		}
		expect(run.sections).toEqual([
			{
				id: 'agents:people',
				label: 'people',
				order: 10,
				results: [
					expect.objectContaining({
						actionId: 'action.alpha',
						id: 'agents:alpha',
						providerId: 'agents',
						providerPriority: 20,
						recommended: true,
						visibility: 'always',
					}),
					expect.objectContaining({
						id: 'agents:beta',
						visibility: 'when-no-results',
					}),
				],
			},
		]);
	});

	it.each(['resolve', 'reject'] as const)(
		'ignores a prior connection that settles by %s after the same request reconnects',
		async (settlement) => {
			const previous = deferred<readonly WorkbenchSearchProviderSection[]>();
			const current = deferred<readonly WorkbenchSearchProviderSection[]>();
			const search = vi
				.fn<WorkbenchSearchProvider['search']>()
				.mockImplementationOnce(() => previous.promise)
				.mockImplementationOnce(() => current.promise);
			const coordinator = new WorkbenchSearchProviderCoordinator(
				[createProvider('agents', { search })],
				'en-US',
			);
			const previousConnection = coordinator.connect();
			coordinator.request({ filter: 'all', query: 'run', queryRevision: 1, sessionId: 1 });
			await flushOperations();
			previousConnection.dispose();
			const currentConnection = coordinator.connect();
			try {
				coordinator.request({ filter: 'all', query: 'run', queryRevision: 1, sessionId: 1 });
				await flushOperations();
				expect(search).toHaveBeenCalledTimes(2);
				if (settlement === 'resolve') previous.resolve([providerSection('stale')]);
				else previous.reject(new Error('obsolete failure'));
				await flushOperations();
				expect(coordinator.getSnapshot().runs).toMatchObject([
					{ phase: 'loading', providerId: 'agents' },
				]);
				current.resolve([providerSection('fresh')]);
				await flushOperations();
				expect(coordinator.getSnapshot().runs).toMatchObject([
					{ phase: 'ready', sections: [{ id: 'agents:fresh' }] },
				]);
			} finally {
				currentConnection.dispose();
			}
		},
	);

	it('aborts active work and publishes an empty snapshot when closed', async () => {
		const pending = deferred<readonly WorkbenchSearchProviderSection[]>();
		let signal: AbortSignal | undefined;
		const coordinator = new WorkbenchSearchProviderCoordinator(
			[
				createProvider('agents', {
					search: (_query, _context, currentSignal) => {
						signal = currentSignal;
						return pending.promise;
					},
				}),
			],
			'en-US',
		);
		const listener = vi.fn<() => void>();
		coordinator.subscribe(listener);
		coordinator.request({ filter: 'all', query: 'run', queryRevision: 1, sessionId: 1 });
		await flushOperations();

		coordinator.close();
		expect(signal?.aborted).toBe(true);
		expect(coordinator.getSnapshot()).toEqual({ request: null, runs: [] });
		expect(listener).toHaveBeenCalledTimes(2);

		pending.resolve([providerSection('late')]);
		await flushOperations();
		expect(coordinator.getSnapshot()).toEqual({ request: null, runs: [] });
	});
});
