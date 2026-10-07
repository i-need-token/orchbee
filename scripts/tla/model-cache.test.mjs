import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, readdir, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { ModelSuccessCache } from './model-cache.mjs';
import { executionPlans, sha256Bytes } from './model-inputs.mjs';

const completed =
	'Model checking completed. No error has been found.\n2 states generated, 1 distinct states found, 0 states left on queue.\nThe depth of the complete state graph search is 1.\nFinished in 1s\n';
const covered = Buffer.from(`${completed}<Advance line 1, col 1 of module Example>: 1:2\n`);
const key = (plan) => JSON.stringify([plan.model.id, plan.variant.name]);
const runtime = {
	jar: 'pinned',
	java: 'runtime',
	helpers: { runner: 'bytes' },
	node: 'version',
	environment: 'digest',
	arguments: ['final'],
	resources: { slots: 2 },
};

async function fixture(t) {
	const root = await mkdtemp(path.join(tmpdir(), 'orchbee-tla-cache-test-'));
	const repository = path.join(root, 'repository');
	const directory = path.join(root, 'cache');
	await mkdir(repository);
	await writeFile(path.join(repository, 'Example.tla'), '---- MODULE Example ----\n====\n');
	await writeFile(path.join(repository, 'Example.cfg'), 'CONSTANT Bound = 1\n');
	const models = ['alpha', 'beta'].map((id) => ({
		id,
		module: 'Example.tla',
		config: 'Example.cfg',
		requiredCoverage: ['Advance'],
		variants: [{ name: 'one' }, { name: 'two', constants: { Bound: 2 } }],
	}));
	const plans = await executionPlans(models, repository, 256, () => 1000);
	const outcomes = new Map(
		plans.map((plan) => [key(plan), { bytes: covered, durationMilliseconds: 100 }]),
	);
	let cache;
	const restart = async () => {
		await cache?.close();
		cache = await ModelSuccessCache.acquire(directory, repository, models);
		return cache;
	};
	t.after(async () => {
		await cache?.close();
		await rm(root, { recursive: true, force: true });
	});
	const publish = async (selected, identity = runtime, validate = async () => {}, signal) => {
		for (const model of models) {
			const group = selected.filter((plan) => plan.model.id === model.id);
			if (group.length) await cache.commitModel(group, identity, outcomes, validate, signal);
		}
	};
	const seed = async () => {
		const current = await restart();
		await current.prepare(plans, runtime);
		await publish(plans);
	};
	const envelope = (id) => path.join(directory, `${sha256Bytes(id)}.json`);
	return { root, repository, directory, plans, models, outcomes, restart, publish, seed, envelope };
}

test('cache reuses complete models across runs, retains exact raw bytes and has one current envelope', async (t) => {
	const f = await fixture(t);
	const raw = Buffer.concat([Buffer.from([0xff, 0xfe, 0x0a]), covered]);
	f.outcomes.set(key(f.plans[0]), { bytes: raw, durationMilliseconds: 321 });
	await f.seed();
	const cache = await f.restart();
	assert.deepEqual(await cache.prepare(f.plans, runtime), []);
	assert.deepEqual(cache.configuration(f.plans[0]).bytes, raw);
	assert.equal(cache.configuration(f.plans[0]).durationMilliseconds, 321);
	assert.equal((await readdir(f.directory)).length, 4); // Two envelopes, owner and live lease.
	await assert.rejects(ModelSuccessCache.acquire(f.directory, f.repository, f.models), {
		code: 'EEXIST',
	});
	assert.equal((await cache.prepare(f.plans, { ...runtime, java: 'changed' })).length, 4);
	assert.equal(cache.configuration(f.plans[0]), undefined);
});

test('every proof/runtime identity category invalidates evidence; helpers bind additions/removals', async (t) => {
	const f = await fixture(t);
	for (const field of Object.keys(runtime)) {
		await f.seed();
		const cache = await f.restart();
		assert.equal(
			(await cache.prepare(f.plans, { ...runtime, [field]: 'changed' })).length,
			4,
			field,
		);
		await assert.rejects(readFile(f.envelope('alpha')), { code: 'ENOENT' });
	}
	for (const field of ['modelConfigSha256', 'workers', 'heapMiB', 'timeoutMilliseconds']) {
		await f.seed();
		const cache = await f.restart();
		const changed = f.plans.map((plan) => ({
			...plan,
			proof: { ...plan.proof, [field]: 'changed' },
		}));
		assert.equal((await cache.prepare(changed, runtime)).length, 4, field);
	}
	for (const field of ['requiredCoverage', 'variants']) {
		await f.seed();
		const cache = await f.restart();
		const changed = f.plans.map((plan) => ({
			...plan,
			model: { ...plan.model, [field]: field === 'variants' ? [{ name: 'changed' }] : ['Changed'] },
		}));
		assert.equal((await cache.prepare(changed, runtime)).length, 4, field);
	}
});

test('one changed configuration reuses its sibling while removing old model visibility before fresh work', async (t) => {
	const f = await fixture(t);
	await f.seed();
	const cache = await f.restart();
	const changed = f.plans.map((plan, index) =>
		index === 0 ? { ...plan, proof: { ...plan.proof, modelConfigSha256: 'changed' } } : plan,
	);
	assert.equal((await cache.prepare(changed, runtime)).length, 1);
	assert.equal(cache.configuration(changed[0]), undefined);
	assert.ok(cache.configuration(changed[1]));
	await assert.rejects(readFile(f.envelope('alpha')), { code: 'ENOENT' });
	await f.publish(changed);
	assert.equal((await readdir(f.directory)).filter((name) => name.endsWith('.tmp')).length, 0);
});

test('forced refresh removes even unstarted selected models before an early failure or cancellation', async (t) => {
	const f = await fixture(t);
	await f.seed();
	const cache = await f.restart();
	assert.equal((await cache.prepare(f.plans, runtime, true)).length, 4);
	for (const model of f.models)
		await assert.rejects(readFile(f.envelope(model.id)), { code: 'ENOENT' });
	const controller = new AbortController();
	controller.abort(new Error('cancelled before rename'));
	await assert.rejects(
		f.publish(f.plans, runtime, async () => {}, controller.signal),
		/cancelled before rename/u,
	);
	for (const model of f.models)
		await assert.rejects(readFile(f.envelope(model.id)), { code: 'ENOENT' });
});

test('corrupt/truncated/log-altered envelopes miss; incomplete output, missing variants and coverage never publish', async (t) => {
	const f = await fixture(t);
	await f.seed();
	const original = await readFile(f.envelope('alpha'), 'utf8');
	for (const bytes of [
		'{',
		original.replace('Advance', 'Missing'),
		original.replace('bW9k', 'YWJj'),
	]) {
		// JSON contains base64 logs: changing the encoded payload or envelope fields must miss.
		await writeFile(
			f.envelope('alpha'),
			bytes === original
				? original.replace('durationMilliseconds":100', 'durationMilliseconds":101')
				: bytes,
		);
		const cache = await f.restart();
		assert.equal(
			(await cache.prepare(f.plans, runtime)).filter((entry) => entry.model === 'alpha').length,
			2,
		);
	}
	const alpha = f.plans.filter((plan) => plan.model.id === 'alpha');
	f.outcomes.set(key(alpha[0]), {
		bytes: Buffer.from('Finished in 1s\n'),
		durationMilliseconds: 1,
	});
	await assert.rejects(f.publish(alpha), /complete successful/u);
	f.outcomes.set(key(alpha[0]), { bytes: Buffer.from(completed), durationMilliseconds: 1 });
	f.outcomes.set(key(alpha[1]), { bytes: Buffer.from(completed), durationMilliseconds: 1 });
	await assert.rejects(f.publish(alpha), /coverage/u);
	f.outcomes.delete(key(alpha[1]));
	await assert.rejects(f.publish(alpha), /Incomplete cache model/u);
	await assert.rejects(readFile(f.envelope('alpha')), { code: 'ENOENT' });
});

test('identity drift prevents publication; a completed different model remains reusable', async (t) => {
	const f = await fixture(t);
	const cache = await f.restart();
	await cache.prepare(f.plans, runtime);
	await assert.rejects(
		f.publish(f.plans, runtime, async () => {
			throw new Error('inputs changed');
		}),
		/inputs changed/u,
	);
	await f.publish(f.plans.filter((plan) => plan.model.id === 'beta'));
	const resumed = await f.restart();
	assert.equal((await resumed.prepare(f.plans, runtime)).length, 2);
	assert.ok(resumed.configuration(f.plans[2]));
});

test('owned retired envelopes and orphan writes are pruned without retaining history', async (t) => {
	const f = await fixture(t);
	await f.seed();
	await writeFile(f.envelope('retired'), '{}');
	await writeFile(`${f.envelope('alpha')}.00000000-0000-0000-0000-000000000000.tmp`, '{}');
	await f.restart();
	assert.equal((await readdir(f.directory)).length, 4);
});

test('unowned directories, repository ancestors, governed paths and links are rejected without deletion', async (t) => {
	const f = await fixture(t);
	await mkdir(f.directory);
	await writeFile(path.join(f.directory, 'user-data'), 'keep');
	await assert.rejects(f.restart(), /not owned/u);
	assert.equal(await readFile(path.join(f.directory, 'user-data'), 'utf8'), 'keep');
	for (const directory of [f.repository, f.root, path.join(f.repository, 'source')])
		await assert.rejects(
			ModelSuccessCache.acquire(directory, f.repository, f.models),
			/separate external directory/u,
		);
	const linked = path.join(f.root, 'linked');
	await symlink(f.directory, linked, process.platform === 'win32' ? 'junction' : 'dir');
	await assert.rejects(ModelSuccessCache.acquire(linked, f.repository, f.models), /links/u);
	if (process.platform === 'win32') {
		const alias = f.repository.toUpperCase();
		assert.notEqual(alias, f.repository);
		await assert.rejects(
			ModelSuccessCache.acquire(path.join(alias, 'unsafe-cache'), f.repository, f.models),
			/canonical spelling/u,
		);
		await assert.rejects(readFile(path.join(f.repository, 'unsafe-cache', 'owner.json')), {
			code: 'ENOENT',
		});
	}
});
