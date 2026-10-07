import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
	changedVerificationPaths,
	readVerificationGraph,
	resolveVerificationScope,
	selectVerificationScope,
	verificationOptions,
} from './verification-scope.mjs';
import { checkSteps } from './run-checks.mjs';

const graph = [
	{ root: 'packages/protocol', name: '@test/protocol', kind: 'pnpm', dependencies: [] },
	{
		root: 'crates/protocol',
		name: 'rust-protocol',
		kind: 'cargo',
		dependencies: ['packages/protocol'],
	},
	{ root: 'apps/worker', name: 'worker', kind: 'cargo', dependencies: ['crates/protocol'] },
	{ root: 'apps/server', name: 'server', kind: 'cargo', dependencies: ['apps/worker'] },
	{ root: 'apps/client', name: 'client', kind: 'cargo', dependencies: ['apps/server'] },
	{ root: 'apps/web', name: '@test/web', kind: 'pnpm', dependencies: ['packages/protocol'] },
	{ root: 'packages/independent', name: '@test/independent', kind: 'pnpm', dependencies: [] },
];

test('one fixed point reaches cross-language transitive consumers and keeps independent suites unexecuted', () => {
	const selected = selectVerificationScope(graph.toReversed(), ['packages/protocol/fixture.bin']);
	assert.equal(selected.all, false);
	assert.deepEqual(selected.cargo, ['client', 'rust-protocol', 'server', 'worker']);
	assert.deepEqual(selected.pnpm, ['@test/protocol', '@test/web']);
	assert.deepEqual(selected.unaffected, ['packages/independent']);
	assert.ok(Object.isFrozen(selected) && Object.isFrozen(selected.cargo));
	assert.deepEqual(selectVerificationScope(graph, []).cargo, []);
	assert.deepEqual(selectVerificationScope(graph, ['apps/client/src/main.rs']).cargo, ['client']);
});

test('manifests, unknown/global paths and unsupported graphs select all rather than omit evidence', () => {
	for (const file of [
		'apps/client/Cargo.toml',
		'packages/protocol/package.json',
		'pnpm-lock.yaml',
		'docs/TESTING.md',
		'nul',
		'apps/retired/src/main.rs',
	])
		assert.equal(selectVerificationScope(graph, [file]).all, true, file);
	for (const invalid of [
		[...graph, graph[0]],
		graph.map((entry, index) =>
			index === 0 ? { ...entry, dependencies: ['apps/client'] } : entry,
		),
		graph.map((entry, index) =>
			index === 0 ? { ...entry, dependencies: ['packages/missing'] } : entry,
		),
		graph.map((entry, index) => (index === 0 ? { ...entry, name: '...*' } : entry)),
	])
		assert.equal(selectVerificationScope(invalid, []).all, true);
});

test('real workspace selection retains foreign fixture, resource, process and discovery consumers', () => {
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	const host = selectVerificationScope(actual, ['apps/terminal-host/src/main.zig']);
	assert.equal(host.all, false, host.reasons.join('\n'));
	for (const name of ['orchbee', 'orchbee-server', 'orchbee-worker'])
		assert.ok(host.cargo.includes(name), name);
	assert.ok(host.pnpm.includes('@orchbee/desktop'));
	for (const [provider, consumers] of [
		[
			'apps/terminal-host',
			['apps/orchbee', 'apps/orchbee-server', 'apps/orchbee-worker', 'apps/desktop'],
		],
		['apps/orchbee-worker', ['apps/orchbee', 'apps/orchbee-server']],
		['apps/orchbee-server', ['apps/orchbee']],
		['apps/web', ['apps/desktop', 'apps/storybook']],
		['packages/terminal-protocol', ['crates/terminal-protocol']],
	]) {
		const selected = selectVerificationScope(actual, [provider + '/src/changed']);
		assert.equal(selected.all, false);
		for (const consumer of consumers) {
			const entry = actual.find((item) => item.root === consumer);
			assert.ok(selected[entry.kind].includes(entry.name), provider + ' must select ' + consumer);
		}
	}
	const protocol = selectVerificationScope(actual, [
		'packages/terminal-protocol/fixtures/sample.bin',
	]);
	assert.ok(protocol.cargo.includes('orchbee-terminal-protocol'));
	assert.ok(protocol.cargo.includes('orchbee'));
	const resources = selectVerificationScope(actual, [
		'apps/desktop/resources/agent-observation-adapters/capsule.json',
	]);
	assert.ok(resources.cargo.includes('orchbee'));
	assert.equal(selectVerificationScope(actual, ['apps/desktop/src/window.ts']).cargo.length, 0);
	assert.ok(
		selectVerificationScope(actual, ['packages/ui/src/component.ts']).pnpm.includes(
			'@orchbee/storybook',
		),
	);
});

test('Git selection covers committed, staged, dirty, untracked, removed and rename paths', (t) => {
	const root = mkdtempSync(path.join(tmpdir(), 'orchbee-verification-scope-'));
	t.after(() => rmSync(root, { recursive: true, force: true }));
	const git = (...args) =>
		execFileSync('git', ['-c', 'core.autocrlf=false', ...args], { cwd: root, encoding: 'utf8' });
	git('init', '--quiet');
	mkdirSync(path.join(root, 'apps/client'), { recursive: true });
	const put = (name, text = 'base\n') => writeFileSync(path.join(root, 'apps/client', name), text);
	for (const name of ['committed.rs', 'staged.rs', 'dirty.rs', 'removed.rs', 'renamed.rs'])
		put(name);
	git('add', '.');
	git(
		'-c',
		'user.name=Fixture',
		'-c',
		'user.email=fixture@example.invalid',
		'commit',
		'--quiet',
		'-m',
		'base',
	);
	const base = git('rev-parse', 'HEAD').trim();
	put('committed.rs', 'changed\n');
	git('add', '.');
	git(
		'-c',
		'user.name=Fixture',
		'-c',
		'user.email=fixture@example.invalid',
		'commit',
		'--quiet',
		'-m',
		'change',
	);
	put('staged.rs', 'changed\n');
	git('add', '.');
	put('dirty.rs', 'changed\n');
	put('未跟踪.rs');
	rmSync(path.join(root, 'apps/client/removed.rs'));
	renameSync(path.join(root, 'apps/client/renamed.rs'), path.join(root, 'apps/client/new-name.rs'));
	const expected = [
		'committed.rs',
		'staged.rs',
		'dirty.rs',
		'removed.rs',
		'renamed.rs',
		'new-name.rs',
		'未跟踪.rs',
	]
		.map((file) => `apps/client/${file}`)
		.toSorted();
	assert.deepEqual(changedVerificationPaths(root, base).paths, expected);
	assert.equal(resolveVerificationScope(root, 'no-such-base').all, true);
	git('update-index', '--assume-unchanged', 'apps/client/dirty.rs');
	assert.throws(() => changedVerificationPaths(root, base), /assume-unchanged/u);
});

test('verification CLI rejects incomplete/duplicate scope and preserves verbose parsing', () => {
	assert.deepEqual(verificationOptions(['--verbose']), {
		verbose: true,
		plan: false,
		changedSince: undefined,
	});
	assert.deepEqual(verificationOptions(['--changed-since', 'main', '--', '--plan']), {
		verbose: false,
		plan: true,
		changedSince: 'main',
	});
	for (const args of [
		['--changed-since'],
		['--changed-since', '--plan'],
		['--plan'],
		['--all'],
		['--changed-since', 'main', '--changed-since', 'HEAD'],
	])
		assert.throws(() => verificationOptions(args));
});

test('affected gate preserves full prerequisites and Node-before-Rust command ordering', () => {
	const selected = selectVerificationScope(graph, ['packages/protocol/src/main.ts']);
	const steps = checkSteps('verify', selected);
	assert.deepEqual(
		steps.slice(0, 5).map((step) => step.name),
		['format:check', 'test:tla:inventory', 'build', 'lint', 'typecheck'],
	);
	const node = steps.findIndex((step) => step.name === 'affected workspace tests');
	const rust = steps.findIndex((step) => step.name === 'affected Rust workspace tests');
	assert.ok(node < rust);
	assert.deepEqual(steps[rust].args, [
		'scripts/run-rust-workspace.mjs',
		'test',
		'--package',
		'client',
		'--package',
		'rust-protocol',
		'--package',
		'server',
		'--package',
		'worker',
	]);
	assert.equal(steps.at(-1).name, 'test:exports');
	assert.deepEqual(checkSteps('verify', { all: true }), checkSteps('verify'));
	assert.equal(checkSteps('test', selectVerificationScope(graph, [])).length, 1);
});
