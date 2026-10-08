import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import {
	copyFileSync,
	existsSync,
	mkdirSync,
	mkdtempSync,
	readFileSync,
	readdirSync,
	rmSync,
	writeFileSync,
} from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import { sha256Bytes } from './model-inputs.mjs';

const repository = path.resolve(import.meta.dirname, '../..');
const tool = JSON.parse(
	readFileSync(path.join(repository, 'scripts/tla/models.json'), 'utf8'),
).tool;
const jar = path.join(repository, '.tools', `tla2tools-${tool.version}.jar`);
const moduleText = `---- MODULE Example ----
EXTENDS Naturals
CONSTANT Bound
VARIABLE n
Init == n = 0
Advance == /\\ n < Bound /\\ n' = n + 1
Spec == Init /\\ [][Advance]_n
TypeOK == n \\in 0..Bound
====
`;

function fixture(t) {
	assert.ok(
		existsSync(jar),
		'Cache CLI evidence requires the pinned TLC jar provisioned by run-models',
	);
	const root = mkdtempSync(path.join(tmpdir(), 'orchbee-tla-cache-cli-'));
	const checkout = path.join(root, 'checkout');
	const scripts = path.join(checkout, 'scripts/tla');
	const directory = path.join(root, 'cache');
	const logs = new Set();
	mkdirSync(scripts, { recursive: true });
	for (const file of readdirSync(import.meta.dirname).filter(
		(entry) => entry.endsWith('.mjs') && !entry.endsWith('.test.mjs'),
	))
		copyFileSync(path.join(import.meta.dirname, file), path.join(scripts, file));
	const models = ['alpha', 'beta'].map((id) => {
		const formal = `apps/${id}/formal/tla`;
		mkdirSync(path.join(checkout, formal), { recursive: true });
		writeFileSync(path.join(checkout, formal, 'Example.tla'), moduleText);
		writeFileSync(
			path.join(checkout, formal, 'Example.cfg'),
			'CONSTANT Bound = 1\nSPECIFICATION Spec\nINVARIANT TypeOK\nCHECK_DEADLOCK FALSE\n',
		);
		writeFileSync(path.join(checkout, `apps/${id}/README.md`), '# Claim\n');
		return {
			id,
			workspace: `apps/${id}`,
			owner: id,
			module: `${formal}/Example.tla`,
			config: `${formal}/Example.cfg`,
			claims: [`apps/${id}/README.md`],
			triggers: [`apps/${id}/src/**`],
			requiredCoverage: ['Advance'],
			variants: [
				{ name: 'one', constants: { Bound: 1 } },
				{ name: 'two', constants: { Bound: 2 } },
			],
		};
	});
	writeFileSync(
		path.join(scripts, 'models.json'),
		JSON.stringify({
			schemaVersion: 1,
			defaultTimeoutMilliseconds: 20000,
			tool,
			sharedTriggers: ['scripts/tla/**'],
			models,
		}),
	);
	const git = spawnSync('git', ['init', '--quiet'], { cwd: checkout, encoding: 'utf8' });
	assert.equal(git.status, 0, git.stderr);
	const environment = { ...process.env, ORCHBEE_TLA2TOOLS_JAR: jar };
	delete environment.NODE_OPTIONS;
	delete environment.ORCHBEE_TLA_TIMEOUT_MILLISECONDS;
	const args = [
		'scripts/tla/run-models.mjs',
		'--models',
		'alpha,beta',
		'--jobs',
		'2',
		'--cache-dir',
		directory,
	];
	function inspect(stdout) {
		for (const match of stdout.matchAll(/^Logs: (.+)$/gmu)) logs.add(match[1].trim());
		return stdout
			.split('\n')
			.filter((line) => line.startsWith('TLA_CACHE_RESULT '))
			.map((line) => JSON.parse(line.slice('TLA_CACHE_RESULT '.length)))
			.at(-1);
	}
	const invoke = (arguments_ = args, env = environment) => {
		const child = spawnSync(process.execPath, arguments_, {
			cwd: checkout,
			env,
			encoding: 'utf8',
			timeout: 90000,
			maxBuffer: 4 * 1024 ** 2,
		});
		assert.ifError(child.error);
		const output = child.stdout + child.stderr;
		return { ...child, output, summary: inspect(child.stdout) };
	};
	t.after(() => {
		for (const log of logs) {
			assert.equal(path.dirname(log), tmpdir());
			assert.ok(path.basename(log).startsWith('orchbee-tla-logs-'));
			rmSync(log, { recursive: true, force: true });
		}
		rmSync(root, { recursive: true, force: true });
	});
	return { checkout, directory, models, args, environment, invoke, inspect };
}

test('real TLC cache executes cold inputs, reuses exact evidence, mixes changed variants and forces a full refresh', (t) => {
	const f = fixture(t);
	const cold = f.invoke();
	assert.equal(cold.status, 0, cold.output);
	assert.deepEqual(cold.summary, {
		freshConfigurations: 4,
		reusedConfigurations: 0,
		complete: true,
	});
	const warm = f.invoke();
	assert.equal(warm.status, 0, warm.output);
	assert.deepEqual(warm.summary, {
		freshConfigurations: 0,
		reusedConfigurations: 4,
		complete: true,
	});
	assert.equal(warm.stdout.includes('\nChecking alpha'), false);
	writeFileSync(
		path.join(f.checkout, 'apps/alpha/README.md'),
		'# Claim\nUnrelated prose does not change a mathematical result.\n',
	);
	const prose = f.invoke();
	assert.equal(prose.status, 0, prose.output);
	assert.equal(prose.summary.reusedConfigurations, 4);
	const inventoryPath = path.join(f.checkout, 'scripts/tla/models.json');
	const inventory = JSON.parse(readFileSync(inventoryPath, 'utf8'));
	inventory.models[0].variants[1].constants.Bound = 3;
	writeFileSync(inventoryPath, JSON.stringify(inventory));
	const mixed = f.invoke();
	assert.equal(mixed.status, 0, mixed.output);
	assert.deepEqual(mixed.summary, {
		freshConfigurations: 1,
		reusedConfigurations: 3,
		complete: true,
	});
	const full = f.invoke([
		'scripts/tla/run-models.mjs',
		'--all',
		'--jobs',
		'2',
		'--cache-dir',
		f.directory,
	]);
	assert.equal(full.status, 0, full.output);
	assert.equal(full.summary.freshConfigurations, 4);
	assert.equal(full.summary.reusedConfigurations, 0);
	const unsupported = f.invoke(f.args, { ...f.environment, NODE_OPTIONS: '--no-warnings' });
	assert.notEqual(unsupported.status, 0);
	assert.match(unsupported.output, /does not support injected NODE_OPTIONS/u);
	const conflict = f.invoke([...f.args, '--run-dir', path.join(f.checkout, 'not-created')]);
	assert.notEqual(conflict.status, 0);
	assert.match(conflict.output, /mutually exclusive/u);
	assert.equal(existsSync(path.join(f.checkout, 'not-created')), false);
});

test('a real failed forced check leaves unstarted models invalidated; uncached execution does not mutate evidence', (t) => {
	const f = fixture(t);
	assert.equal(f.invoke().status, 0);
	const before = readdirSync(f.directory).map((file) => [
		file,
		readFileSync(path.join(f.directory, file), 'utf8'),
	]);
	const uncached = f.invoke(['scripts/tla/run-models.mjs', '--all', '--jobs', '2']);
	assert.equal(uncached.status, 0, uncached.output);
	assert.deepEqual(
		readdirSync(f.directory).map((file) => [
			file,
			readFileSync(path.join(f.directory, file), 'utf8'),
		]),
		before,
	);
	writeFileSync(
		path.join(f.checkout, f.models[0].module),
		moduleText.replace('n \\in 0..Bound', 'n < Bound'),
	);
	const failed = f.invoke([
		'scripts/tla/run-models.mjs',
		'--all',
		'--jobs',
		'1',
		'--cache-dir',
		f.directory,
	]);
	assert.notEqual(failed.status, 0);
	assert.equal(failed.summary.complete, false);
	assert.match(failed.stdout, /TLA_SKIPPED.*beta/u);
	for (const model of f.models)
		assert.equal(existsSync(path.join(f.directory, `${sha256Bytes(model.id)}.json`)), false);
});

test('editing staged source during real TLC makes the invocation incomplete and suppresses publication', async (t) => {
	const f = fixture(t);
	const child = spawn(process.execPath, f.args, {
		cwd: f.checkout,
		env: f.environment,
		stdio: ['ignore', 'pipe', 'pipe'],
	});
	let stdout = '';
	let stderr = '';
	let changed = false;
	const timer = setTimeout(() => child.kill('SIGKILL'), 90000);
	child.stdout.on('data', (data) => {
		stdout += data;
		if (!changed && stdout.includes('Checking alpha/one')) {
			changed = true;
			writeFileSync(
				path.join(f.checkout, f.models[0].module),
				`${moduleText}\n\\* concurrent source edit\n`,
			);
		}
	});
	child.stderr.on('data', (data) => {
		stderr += data;
	});
	const status = await new Promise((resolve, reject) => {
		child.once('error', reject);
		child.once('close', resolve);
	});
	clearTimeout(timer);
	const summary = f.inspect(stdout);
	assert.ok(changed);
	assert.notEqual(status, 0, stdout + stderr);
	assert.equal(summary.complete, false);
	assert.match(summary.integrityFailure, /identity changed/u);
	for (const model of f.models)
		assert.equal(existsSync(path.join(f.directory, `${sha256Bytes(model.id)}.json`)), false);
});

test('real reuse tracks recursive helper data, unusual names and member additions/removals', (t) => {
	const f = fixture(t);
	const directory = path.join(f.checkout, 'scripts/tla/runtime.md');
	mkdirSync(directory);
	const data = path.join(directory, 'settings.md');
	const unusual = path.join(f.checkout, 'scripts/tla/__proto__');
	writeFileSync(data, '{"mode":1}\n');
	writeFileSync(unusual, 'first\n');
	assert.equal(f.invoke().status, 0);
	const warm = f.invoke();
	assert.equal(warm.status, 0, warm.output);
	assert.equal(warm.summary.reusedConfigurations, 4);
	for (const mutate of [
		() => writeFileSync(data, '{"mode":2}\n'),
		() => writeFileSync(unusual, 'second\n'),
		() => writeFileSync(path.join(directory, 'added.bin'), 'added\n'),
		() => rmSync(path.join(directory, 'added.bin')),
	]) {
		mutate();
		const result = f.invoke();
		assert.equal(result.status, 0, result.output);
		assert.equal(result.summary.freshConfigurations, 4);
		assert.equal(result.summary.reusedConfigurations, 0);
	}
});

test('a completed model publishes reusable real evidence when a later model fails', (t) => {
	const f = fixture(t);
	writeFileSync(
		path.join(f.checkout, f.models[1].module),
		moduleText.replace('n \\in 0..Bound', 'n < Bound'),
	);
	const failed = f.invoke([
		'scripts/tla/run-models.mjs',
		'--models',
		'alpha,beta',
		'--jobs',
		'1',
		'--cache-dir',
		f.directory,
	]);
	assert.notEqual(failed.status, 0, failed.output);
	assert.equal(failed.summary.complete, false);
	assert.equal(existsSync(path.join(f.directory, sha256Bytes('alpha') + '.json')), true);
	assert.equal(existsSync(path.join(f.directory, sha256Bytes('beta') + '.json')), false);
	const reused = f.invoke([
		'scripts/tla/run-models.mjs',
		'--models',
		'alpha',
		'--jobs',
		'1',
		'--cache-dir',
		f.directory,
	]);
	assert.equal(reused.status, 0, reused.output);
	assert.deepEqual(reused.summary, {
		freshConfigurations: 0,
		reusedConfigurations: 2,
		complete: true,
	});
});

test('a complete real model publishes before a held peer and survives later cancellation', async (t) => {
	const f = fixture(t);
	const runner = path.join(f.checkout, 'scripts/tla/run-models.mjs');
	// Hold the next model at the adapter boundary. The fixture helper bytes are part of
	// the identity on both runs; alpha still executes the real pinned TLC unchanged.
	writeFileSync(
		runner,
		readFileSync(runner, 'utf8').replace(
			'async function runVariant(plan, recordFailure, context) {',
			`async function runVariant(plan, recordFailure, context) {
			if (plan.model.id === 'beta') {
				const { watch } = await import('node:fs');
				await new Promise((resolve) => {
					const watcher = watch(repositoryRoot, (_event, name) => {
						if (name !== 'cancel-witness') return;
						watcher.close();
						process.emit('SIGTERM');
						resolve();
					});
					process.stdout.write('TLA_WITNESS_PEER_HELD\\n');
				});
				context.signal.throwIfAborted();
			}`,
		),
	);
	const args = [...f.args];
	args[args.indexOf('--jobs') + 1] = '1';
	const child = spawn(process.execPath, args, {
		cwd: f.checkout,
		env: f.environment,
		stdio: ['ignore', 'pipe', 'pipe'],
	});
	let stdout = '';
	let stderr = '';
	let held = false;
	let publishedWhileHeld = false;
	const timer = setTimeout(() => child.kill('SIGKILL'), 90000);
	child.stdout.on('data', (data) => {
		stdout += data;
		if (held || !stdout.includes('TLA_WITNESS_PEER_HELD')) return;
		held = true;
		publishedWhileHeld = existsSync(path.join(f.directory, sha256Bytes('alpha') + '.json'));
		writeFileSync(path.join(f.checkout, 'cancel-witness'), 'cancel\n');
	});
	child.stderr.on('data', (data) => {
		stderr += data;
	});
	const status = await new Promise((resolve, reject) => {
		child.once('error', reject);
		child.once('close', resolve);
	}).finally(() => clearTimeout(timer));
	const summary = f.inspect(stdout);
	assert.ok(held, stdout + stderr);
	assert.ok(publishedWhileHeld, 'Completed alpha must be visible before beta finishes');
	assert.notEqual(status, 0, stdout + stderr);
	assert.equal(summary.complete, false);
	assert.match(stdout + stderr, /TLC checking cancelled/u);
	assert.equal(existsSync(path.join(f.directory, sha256Bytes('beta') + '.json')), false);
	const reused = f.invoke([
		'scripts/tla/run-models.mjs',
		'--models',
		'alpha',
		'--jobs',
		'1',
		'--cache-dir',
		f.directory,
	]);
	assert.equal(reused.status, 0, reused.output);
	assert.equal(reused.summary.reusedConfigurations, 2);
	assert.equal(reused.summary.freshConfigurations, 0);
});

test('final identity validation rejects drift after publication while retaining evidence for its original inputs', (t) => {
	const f = fixture(t);
	const runner = path.join(f.checkout, 'scripts/tla/run-models.mjs');
	const anchor = '\n\t} catch (error) {\n\t\texecutionError = error;';
	const text = readFileSync(runner, 'utf8');
	assert.equal(text.split(anchor).length, 2);
	writeFileSync(
		runner,
		text.replace(
			anchor,
			`
		const { writeFile: mutateInput, readFile: readInput } = await import('node:fs/promises');
		const file = path.join(repositoryRoot, 'apps/alpha/formal/tla/Example.tla');
		if (!(await readInput(path.join(repositoryRoot, 'witness-complete'), 'utf8').catch(() => '')).trim())
			await mutateInput(file, (await readInput(file, 'utf8')) + '\\n\\\\* after publication\\n');
		${anchor}`,
		),
	);
	const failed = f.invoke();
	assert.notEqual(failed.status, 0, failed.output);
	assert.equal(failed.summary.complete, false);
	assert.match(failed.summary.integrityFailure, /identity changed/u);
	for (const model of f.models)
		assert.equal(existsSync(path.join(f.directory, sha256Bytes(model.id) + '.json')), true);
	writeFileSync(path.join(f.checkout, f.models[0].module), moduleText);
	writeFileSync(path.join(f.checkout, 'witness-complete'), 'done\n');
	const restored = f.invoke();
	assert.equal(restored.status, 0, restored.output);
	assert.equal(restored.summary.reusedConfigurations, 4);
	assert.equal(restored.summary.freshConfigurations, 0);
});

test('cancellation after the sole model configurations close suppresses its cache publication', (t) => {
	const f = fixture(t);
	const adapter = path.join(f.checkout, 'scripts/tla/model-cache.mjs');
	const text = readFileSync(adapter, 'utf8');
	const anchor = '\t\tsignal?.throwIfAborted();';
	assert.equal(text.split(anchor).length, 2);
	writeFileSync(
		adapter,
		text.replace(
			anchor,
			"\t\tprocess.stdout.write('TLA_WITNESS_LAST_MODEL_READY\\n');\n\t\tprocess.emit('SIGTERM');\n" +
				anchor,
		),
	);
	const args = [...f.args];
	args[args.indexOf('--models') + 1] = 'alpha';
	args[args.indexOf('--jobs') + 1] = '1';
	const cancelled = f.invoke(args);
	assert.match(cancelled.stdout, /TLA_WITNESS_LAST_MODEL_READY/u);
	assert.equal(cancelled.summary.freshConfigurations, 2);
	assert.equal(cancelled.summary.complete, false);
	assert.notEqual(cancelled.status, 0);
	assert.match(cancelled.output, /TLC checking cancelled/u);
	assert.equal(existsSync(path.join(f.directory, sha256Bytes('alpha') + '.json')), false);
});

test('a cached removal-only selection prunes retired evidence without invoking Java', (t) => {
	const f = fixture(t);
	assert.equal(f.invoke().status, 0);
	for (const args of [
		['add', '.'],
		[
			'-c',
			'user.name=Fixture',
			'-c',
			'user.email=fixture@example.invalid',
			'commit',
			'--quiet',
			'-m',
			'base',
		],
	]) {
		const result = spawnSync('git', args, { cwd: f.checkout, encoding: 'utf8' });
		assert.equal(result.status, 0, result.stderr);
	}
	const file = path.join(f.checkout, 'scripts/tla/models.json');
	const current = JSON.parse(readFileSync(file, 'utf8'));
	current.models.pop();
	writeFileSync(file, JSON.stringify(current));
	const result = f.invoke(
		['scripts/tla/run-models.mjs', '--changed-since', 'HEAD', '--cache-dir', f.directory],
		{ ...f.environment, ORCHBEE_TLA_JAVA: 'nonexistent-java' },
	);
	assert.equal(result.status, 0, result.output);
	assert.match(result.stdout, /No TLA\+ model is affected/u);
	assert.equal(existsSync(path.join(f.directory, sha256Bytes('beta') + '.json')), false);
	assert.equal(existsSync(path.join(f.directory, sha256Bytes('alpha') + '.json')), true);
});
