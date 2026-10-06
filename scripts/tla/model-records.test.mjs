import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, readdir, rename, rm, writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { HistoricalModelFailure, runModelChecks } from './model-checks.mjs';
import { executionPlans, stageInputs } from './model-inputs.mjs';
import { ModelRunRecords } from './model-records.mjs';

const completed =
	'Model checking completed. No error has been found.\n2 states generated, 1 distinct states found, 0 states left on queue.\nThe depth of the complete state graph search is 1.\nFinished in 1s\n';
const covered = `${completed}<Advance line 1, col 1 of module Example>: 1:2\n`;

async function fixture(t) {
	const root = await mkdtemp(path.join(os.tmpdir(), 'orchbee-tla-records-test-'));
	const repository = path.join(root, 'repository');
	const directory = path.join(root, 'run');
	await mkdir(repository);
	await writeFile(path.join(repository, 'Example.tla'), '---- MODULE Example ----\n====\n');
	await writeFile(path.join(repository, 'Example.cfg'), 'CONSTANT Bound = 1\n');
	const models = ['alpha', 'beta'].map((id) => ({
		id,
		module: 'Example.tla',
		config: 'Example.cfg',
		requiredCoverage: ['Advance'],
	}));
	const plans = await executionPlans(models, repository, 256, () => 1000);
	const context = {
		repository,
		revision: 'frozen',
		node: process.version,
		java: 'Java 17',
		heapMiB: 256,
	};
	const acquire = () => ModelRunRecords.acquire(directory, repository, context, plans);
	let records = await acquire();
	t.after(async () => {
		if (records) await records.close();
		await rm(root, { recursive: true, force: true });
	});
	async function restart() {
		await records.close();
		records = undefined;
		records = await acquire();
		return records;
	}
	async function commit(plan, output = covered, status = 'passed') {
		const label = `${plan.model.id}--${plan.variant.name}--test`;
		const inputDirectory = path.join(directory, 'inputs', label);
		await mkdir(inputDirectory);
		const staged = await stageInputs(plan, inputDirectory);
		const inputs = staged.map((input) => ({
			path: `inputs/${label}/${input.name}`,
			sha256: input.sha256,
		}));
		const log = `logs/${label}.log`;
		await writeFile(path.join(directory, log), output);
		const report = {
			model: plan.model.id,
			variant: plan.variant.name,
			modelConfigSha256: plan.proof.modelConfigSha256,
			workers: 1,
			heapMiB: 256,
			durationMilliseconds: 1000,
			...(status === 'passed'
				? { generatedStates: 2, distinctStates: 1, statesLeft: 0, depth: 1 }
				: {}),
			logPath: path.join(directory, log),
		};
		await records.commitConfiguration(plan, {
			report,
			status,
			message: status === 'passed' ? null : 'original failure',
			log,
			inputs,
		});
	}
	return {
		root,
		repository,
		directory,
		models,
		plans,
		context,
		acquire,
		restart,
		commit,
		get records() {
			return records;
		},
	};
}

test('persisted model coverage failure survives cached configuration continuation while another model runs', async (t) => {
	const f = await fixture(t);
	await f.records.request(f.models);
	await f.commit(f.plans[0], completed);
	await f.records.commitModel(f.models[0], {
		status: 'failed',
		error: new Error('alpha did not satisfy required TLC action coverage'),
	});
	const records = await f.restart();
	const calls = [];
	await assert.rejects(
		runModelChecks(
			f.models,
			2,
			async (model) => {
				const plan = f.plans.find((entry) => entry.model.id === model.id);
				const cached = records.configuration(plan);
				if (cached) {
					calls.push(`reuse:${model.id}`);
					return cached.output;
				}
				calls.push(`run:${model.id}`);
				await f.commit(plan);
				return covered;
			},
			undefined,
			{
				previousModelFailures: records.previousModelFailures(),
				onModelOutcome: (model, outcome) => records.commitModel(model, outcome),
			},
		),
		HistoricalModelFailure,
	);
	assert.deepEqual(calls, ['reuse:alpha', 'run:beta']);
	assert.deepEqual(records.summary().models, [
		{ model: 'alpha', status: 'failed' },
		{ model: 'beta', status: 'passed' },
	]);
	assert.equal(records.summary().complete, false);
	assert.equal((await f.restart()).summary().failedModels, 1);
});

test('historical configuration failure allows pending work but never automatic retry or success', async (t) => {
	const f = await fixture(t);
	await f.records.request([f.models[0]]);
	await f.commit(f.plans[0], 'failure log', 'failed');
	const records = await f.restart();
	await records.request([f.models[1]]);
	let launched = 0;
	await assert.rejects(
		runModelChecks(
			f.models,
			1,
			async (model) => {
				const plan = f.plans.find((entry) => entry.model.id === model.id);
				const cached = records.configuration(plan);
				if (cached?.record.status === 'failed')
					throw new HistoricalModelFailure(cached.record.message);
				launched += 1;
				await f.commit(plan);
				return covered;
			},
			undefined,
			{ onModelOutcome: (model, outcome) => records.commitModel(model, outcome) },
		),
		HistoricalModelFailure,
	);
	assert.equal(launched, 1);
	assert.equal(records.summary().failedModels, 1);
	assert.equal(records.summary().passedModels, 1);
	await assert.rejects(f.commit(f.plans[0]), /EEXIST/u);
});

test('all committed proof integrity is checked before a continuation is admitted', async (t) => {
	for (const corruption of ['log', 'missing-log', 'input', 'schema', 'proof', 'report', 'model']) {
		await t.test(corruption, async (context) => {
			const f = await fixture(context);
			await f.records.request(f.models);
			await f.commit(f.plans[0]);
			await f.records.commitModel(f.models[0], { status: 'passed' });
			const recordFile = path.join(f.directory, 'records/alpha--default.json');
			const record = JSON.parse(await readFile(recordFile, 'utf8'));
			if (corruption === 'log') await writeFile(path.join(f.directory, record.log), 'truncated');
			if (corruption === 'missing-log') await rm(path.join(f.directory, record.log));
			if (corruption === 'input')
				await writeFile(path.join(f.directory, record.inputs[0].path), 'changed');
			if (corruption === 'schema') {
				record.future = true;
				await writeFile(recordFile, JSON.stringify(record));
			}
			if (corruption === 'proof') {
				record.proof.modelConfigSha256 = 'stale';
				await writeFile(recordFile, JSON.stringify(record));
			}
			if (corruption === 'report') {
				record.report.statesLeft = 1;
				await writeFile(recordFile, JSON.stringify(record));
			}
			if (corruption === 'model') {
				const file = path.join(f.directory, 'models/alpha.json');
				const aggregate = JSON.parse(await readFile(file, 'utf8'));
				aggregate.configurations = [];
				await writeFile(file, JSON.stringify(aggregate));
			}
			await assert.rejects(f.restart());
			assert.ok(!(await readdir(f.directory)).includes('.tla-run.lock'));
		});
	}
});

test('identity changes, ambiguous leases and checkout-contained state are refused', async (t) => {
	const f = await fixture(t);
	await assert.rejects(f.acquire(), /writer lease/u);
	await assert.rejects(
		ModelRunRecords.acquire(
			path.join(f.repository, '.tools/run'),
			f.repository,
			f.context,
			f.plans,
		),
		/external task directory/u,
	);
	await f.records.close();
	for (const changed of [
		{ ...f.context, node: 'different' },
		{ ...f.context, java: 'different' },
		{ ...f.context, revision: 'different' },
		{ ...f.context, heapMiB: 512 },
	]) {
		await assert.rejects(
			ModelRunRecords.acquire(f.directory, f.repository, changed, f.plans),
			/identity changed/u,
		);
	}
	// Reacquire so the fixture's finalizer owns a current lease.
	await f.restart().catch(() => {});
});

test('uncommitted temporary records are pending and success requires complete TLC output', async (t) => {
	const f = await fixture(t);
	await writeFile(path.join(f.directory, 'records/alpha--default.json.partial.tmp'), '{');
	const records = await f.restart();
	assert.equal(records.configuration(f.plans[0]), undefined);
	await assert.rejects(
		f.commit(f.plans[0], '<Advance line 1, col 1 of module Example>: 1:2\n'),
		/complete successful/u,
	);
	assert.equal(records.configuration(f.plans[0]), undefined);
});

test('moving a run root cannot transfer its committed run identity', async (t) => {
	const f = await fixture(t);
	await f.records.close();
	const moved = path.join(f.root, 'moved');
	await rename(f.directory, moved);
	await assert.rejects(
		ModelRunRecords.acquire(moved, f.repository, f.context, f.plans),
		/identity changed/u,
	);
});

test('failed configuration reports enforce the same identity, resource and log fields as passes', async (t) => {
	for (const [field, value] of [
		['model', 'beta'],
		['variant', 'other'],
		['modelConfigSha256', 'stale'],
		['workers', 8],
		['heapMiB', 999],
		['logPath', 'C:/unrelated.log'],
		['durationMilliseconds', -99],
		['statesLeft', 0],
	]) {
		await t.test(field, async (context) => {
			const f = await fixture(context);
			await f.commit(f.plans[0], 'failure log', 'failed');
			const file = path.join(f.directory, 'records/alpha--default.json');
			const record = JSON.parse(await readFile(file, 'utf8'));
			record.report[field] = value;
			await writeFile(file, JSON.stringify(record));
			await assert.rejects(f.restart(), /Invalid TLC outcome report/u);
		});
	}
});
