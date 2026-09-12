import assert from 'node:assert/strict';
import test from 'node:test';

import {
	formatTlcOutput,
	modelConcurrency,
	modelHeapMiB,
	modelWorkers,
	runModelChecks,
	writeTlcOutput,
} from './model-checks.mjs';

function deferred() {
	let resolve;
	const promise = new Promise((done) => {
		resolve = done;
	});
	return { promise, resolve };
}

function model(id, variants = [{ name: 'default' }]) {
	return { id, variants, requiredCoverage: ['Advance'] };
}

const covered = '<Advance line 1, col 1 of module Example>: 1:2\n';

test('resource defaults scale with processors and aggregate JVM memory', () => {
	assert.equal(modelConcurrency(undefined, 8, 16 * 1024 ** 3), 3);
	assert.equal(modelConcurrency(undefined, 2, 16 * 1024 ** 3), 2);
	assert.equal(modelConcurrency(undefined, 8, 7 * 1024 ** 3), 1);
	assert.equal(modelConcurrency(undefined, 24, 24 * 1024 ** 3), 5);
	assert.equal(modelConcurrency('1', 8, 16 * 1024 ** 3), 1);
	assert.equal(modelConcurrency('8', 24, 24 * 1024 ** 3), 8);
	assert.equal(modelHeapMiB(1, 24 * 1024 ** 3), 2048);
	assert.ok((modelHeapMiB(8, 24 * 1024 ** 3) + 512) * 8 <= 24 * 1024 * 0.6);
	assert.throws(() => modelConcurrency('32', 24, 24 * 1024 ** 3), /memory budget/u);
	for (const value of ['0', '-1', '1.5', 'NaN', '', '9007199254740992']) {
		assert.throws(() => modelConcurrency(value, 8, 16 * 1024 ** 3), /positive safe integer/u);
	}
});

test('model worker count defaults to one and each completed output uses one adjacent write', async () => {
	assert.equal(modelWorkers({}), 1);
	assert.equal(modelWorkers({ workers: 2 }), 2);
	const report = { model: 'example', variant: 'default', workers: 2 };
	const writes = [];
	const stream = {
		write(output, callback) {
			writes.push(output);
			callback();
		},
	};
	await writeTlcOutput(stream, 'example', 'states complete', report);
	await writeTlcOutput(stream, 'failed', 'counterexample\n');
	assert.deepEqual(writes, [
		`\nTLC output for example:\nstates complete\nTLA_RESULT ${JSON.stringify(report)}\n`,
		'\nTLC output for failed:\ncounterexample\n',
	]);
	assert.equal(formatTlcOutput('example', 'states complete', report), writes[0]);
});

test('same-model variants share slots and aggregate their local coverage only after both pass', async () => {
	const firstRelease = deferred();
	const secondRelease = deferred();
	const bothStarted = deferred();
	const nextStarted = deferred();
	const calls = [];
	let running = 0;
	let maximum = 0;
	let variantStarts = 0;
	let settled = false;
	const checking = runModelChecks(
		[model('variants', [{ name: 'a' }, { name: 'b' }]), model('next')],
		2,
		async (selected, variant) => {
			calls.push(`${selected.id}/${variant.name}`);
			running += 1;
			maximum = Math.max(maximum, running);
			try {
				if (selected.id === 'variants') {
					variantStarts += 1;
					if (variantStarts === 2) bothStarted.resolve();
					await (variant.name === 'a' ? firstRelease.promise : secondRelease.promise);
					return variant.name === 'a' ? '' : covered;
				}
				nextStarted.resolve();
				return covered;
			} finally {
				running -= 1;
			}
		},
	).finally(() => {
		settled = true;
	});
	await bothStarted.promise;
	assert.deepEqual(calls, ['variants/a', 'variants/b']);
	assert.equal(maximum, 2);
	firstRelease.resolve();
	await Promise.race([
		nextStarted.promise,
		checking.then(() => assert.fail('coverage ran before the second variant completed')),
	]);
	assert.deepEqual(calls, ['variants/a', 'variants/b', 'next/default']);
	assert.equal(settled, false);
	secondRelease.resolve();
	assert.equal(await checking, 3);
});

test('coverage from one model cannot satisfy another model', async () => {
	await assert.rejects(
		runModelChecks(
			[model('covered', [{ name: 'a' }, { name: 'b' }]), model('uncovered')],
			2,
			async (selected, variant) =>
				selected.id === 'covered' && variant.name === 'b' ? covered : '',
		),
		/uncovered did not satisfy required TLC action coverage/u,
	);
});

test('a variant failure stops new jobs, drains its active sibling, and preserves the first error', async () => {
	const active = deferred();
	const activeFinished = deferred();
	const activeStarted = deferred();
	const failureRecorded = deferred();
	const failureLogDrained = deferred();
	const expected = new Error('TLC timed out');
	const siblingError = new Error('active sibling failed later');
	const calls = [];
	let settled = false;
	const skipped = [];
	const checking = runModelChecks(
		[
			model('variants', [{ name: 'failing' }, { name: 'active' }, { name: 'queued' }]),
			model('later'),
		],
		2,
		async (selected, variant, recordFailure) => {
			calls.push(`${selected.id}/${variant.name}`);
			if (variant.name === 'failing') {
				await activeStarted.promise;
				recordFailure(expected);
				failureRecorded.resolve();
				await failureLogDrained.promise;
				throw expected;
			}
			activeStarted.resolve();
			await active.promise;
			activeFinished.resolve();
			throw siblingError;
		},
		(selected, variant) => skipped.push(`${selected.id}/${variant.name}`),
	).finally(() => {
		settled = true;
	});
	const rejection = assert.rejects(checking, (error) => error === expected);
	await failureRecorded.promise;
	assert.equal(settled, false);
	active.resolve();
	await activeFinished.promise;
	assert.deepEqual(calls, ['variants/failing', 'variants/active']);
	assert.equal(settled, false);
	failureLogDrained.resolve();
	await rejection;
	assert.deepEqual(calls, ['variants/failing', 'variants/active']);
	assert.deepEqual(skipped, ['variants/queued', 'later/default']);
});

test('independent checks can fill more than two slots without sharing model coverage', async () => {
	const release = deferred();
	const started = [];
	const checks = Array.from({ length: 8 }, (_, index) => model(`model-${index}`));
	const checking = runModelChecks(checks, 8, async (selected) => {
		started.push(selected.id);
		await release.promise;
		return covered;
	});
	assert.equal(started.length, 8);
	release.resolve();
	assert.equal(await checking, 8);
});
