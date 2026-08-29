import { describe, expect, it, vi } from 'vitest';

import { DesktopEnvironmentRegistry } from '../DesktopEnvironmentRegistry';
import {
	EnvironmentIdSchema,
	EnvironmentRecordSchema,
	createEmptyEnvironmentRegistryDocument,
	type EnvironmentRecord,
	type EnvironmentRegistryDocument,
} from '../environment-registry-contract';
import type { EnvironmentRegistryStore } from '../FileEnvironmentRegistryStore';

describe('DesktopEnvironmentRegistry', () => {
	it('loads once, serializes creates, and exposes detached stable-order snapshots', async () => {
		const fixture = createStoreFixture();
		const registry = new DesktopEnvironmentRegistry(fixture.store);
		await expect(registry.list()).resolves.toEqual({
			kind: 'unavailable',
			reason: 'not-initialized',
		});
		await expect(registry.initialize()).resolves.toMatchObject({
			document: { environments: [], revision: 0 },
			kind: 'ready',
		});

		const first = environment(1, 1);
		const second = environment(2, 2);
		const [firstResult, secondResult] = await Promise.all([
			registry.create(first),
			registry.create(second),
		]);

		expect(firstResult).toEqual({ environment: first, kind: 'created', revision: 1 });
		expect(secondResult).toEqual({ environment: second, kind: 'created', revision: 2 });
		expect(fixture.store.commit.mock.calls.map(([revision]) => revision)).toEqual([0, 1]);
		const listed = await registry.list();
		expect(listed).toEqual({ kind: 'ready', value: [first, second] });
		if (listed.kind !== 'ready') throw new Error('Expected a ready registry.');
		(listed.value as EnvironmentRecord[]).splice(0);
		await expect(registry.list()).resolves.toEqual({ kind: 'ready', value: [first, second] });
		await expect(registry.get(first.environmentId)).resolves.toEqual({
			kind: 'ready',
			value: first,
		});
	});

	it('returns idempotent and conflict results without writing', async () => {
		const fixture = createStoreFixture();
		const registry = new DesktopEnvironmentRegistry(fixture.store);
		await registry.initialize();
		const original = environment(1, 1);
		await registry.create(original);
		fixture.store.commit.mockClear();

		await expect(registry.create(original)).resolves.toEqual({
			environment: original,
			kind: 'existing',
			revision: 1,
		});
		await expect(registry.create(environment(1, 2))).resolves.toMatchObject({
			environment: original,
			kind: 'conflict',
			reason: 'environment-id',
			revision: 1,
		});
		await expect(registry.create(environment(2, 1))).resolves.toMatchObject({
			environment: original,
			kind: 'conflict',
			reason: 'ssh-target',
			revision: 1,
		});
		expect(fixture.store.commit).not.toHaveBeenCalled();
	});

	it('refreshes from a store revision conflict without silently reordering the request', async () => {
		const fixture = createStoreFixture();
		const registry = new DesktopEnvironmentRegistry(fixture.store);
		await registry.initialize();
		const concurrent = environment(1, 1);
		const concurrentDocument = documentWith(concurrent);
		fixture.store.commit.mockResolvedValueOnce({
			document: concurrentDocument,
			kind: 'conflict',
		});

		await expect(registry.create(environment(2, 2))).resolves.toEqual({
			kind: 'conflict',
			reason: 'stale-revision',
			revision: 1,
		});
		await expect(registry.list()).resolves.toEqual({
			kind: 'ready',
			value: [concurrent],
		});
	});

	it('blocks after an indeterminate rename until an explicit reload reconciles the file', async () => {
		const fixture = createStoreFixture();
		const registry = new DesktopEnvironmentRegistry(fixture.store);
		await registry.initialize();
		const requested = environment(1, 1);
		fixture.store.commit.mockImplementationOnce(async (_revision, document) => {
			fixture.setDocument(document);
			return { kind: 'indeterminate', reason: 'durability-unconfirmed' };
		});

		await expect(registry.create(requested)).resolves.toEqual({
			kind: 'indeterminate',
			reason: 'durability-unconfirmed',
		});
		await expect(registry.list()).resolves.toEqual({
			kind: 'unavailable',
			reason: 'commit-indeterminate',
		});
		await expect(registry.initialize()).resolves.toMatchObject({
			document: { environments: [requested], revision: 1 },
			kind: 'ready',
		});
		await expect(registry.create(requested)).resolves.toEqual({
			environment: requested,
			kind: 'existing',
			revision: 1,
		});
	});

	it('preserves the readable snapshot after a pre-commit write failure', async () => {
		const fixture = createStoreFixture();
		const registry = new DesktopEnvironmentRegistry(fixture.store);
		await registry.initialize();
		fixture.store.commit.mockResolvedValueOnce({ kind: 'unavailable', reason: 'write-failed' });

		await expect(registry.create(environment(1, 1))).resolves.toEqual({
			kind: 'unavailable',
			reason: 'write-failed',
		});
		await expect(registry.list()).resolves.toEqual({ kind: 'ready', value: [] });
	});

	it('blocks reads when initialization cannot validate the persisted registry', async () => {
		const fixture = createStoreFixture();
		fixture.store.load.mockResolvedValueOnce({ kind: 'unavailable', reason: 'invalid-record' });
		const registry = new DesktopEnvironmentRegistry(fixture.store);

		await expect(registry.initialize()).resolves.toEqual({
			kind: 'unavailable',
			reason: 'invalid-record',
		});
		await expect(
			registry.get(EnvironmentIdSchema.parse(`environment:${'1'.repeat(32)}`)),
		).resolves.toEqual({ kind: 'unavailable', reason: 'invalid-record' });
	});
});

function createStoreFixture() {
	let document = createEmptyEnvironmentRegistryDocument();
	const store = {
		commit: vi.fn<EnvironmentRegistryStore['commit']>(async (expectedRevision, nextDocument) => {
			if (document.revision !== expectedRevision) return { document, kind: 'conflict' };
			document = nextDocument;
			return { kind: 'committed' };
		}),
		load: vi.fn<EnvironmentRegistryStore['load']>(async () => ({ document, kind: 'ready' })),
	} satisfies EnvironmentRegistryStore;
	return {
		setDocument: (nextDocument: EnvironmentRegistryDocument) => {
			document = nextDocument;
		},
		store,
	};
}

function documentWith(record: EnvironmentRecord): EnvironmentRegistryDocument {
	return {
		environments: [record],
		revision: 1,
		schemaVersion: 1,
	};
}

function environment(identity: number, target: number): EnvironmentRecord {
	return EnvironmentRecordSchema.parse({
		environmentId: `environment:${hexIdentity(identity)}`,
		name: `Environment ${identity}`,
		provider: {
			kind: 'ssh',
			runtimeInstanceId: hexIdentity(identity + 10),
			target: { configurationFilePath: null, destination: `build-${target}` },
		},
	});
}

function hexIdentity(value: number): string {
	return value.toString(16).padStart(32, '0');
}
