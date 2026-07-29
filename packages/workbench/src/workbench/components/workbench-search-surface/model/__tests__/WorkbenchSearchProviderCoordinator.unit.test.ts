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
