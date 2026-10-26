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
		['apps/orchbee-settings', ['apps/desktop']],
	]) {
		const selected = selectVerificationScope(actual, [provider + '/src/changed']);
		assert.equal(selected.all, false);
		for (const consumer of consumers) {
			const entry = actual.find((item) => item.root === consumer);
			assert.ok(selected[entry.kind].includes(entry.name), provider + ' must select ' + consumer);
		}
	}
	for (const changed of [
		'schemas/terminal/v1/fixtures/sample.bin',
		'schemas/terminal/v1/constants.json',
		'schemas/terminal/README.md',
		'schemas/terminal/references/protocol.md',
	]) {
		const protocol = selectVerificationScope(actual, [changed]);
		assert.equal(protocol.all, false, protocol.reasons.join('\n'));
		for (const name of ['orchbee-terminal-protocol', 'orchbee', 'orchbee-server', 'orchbee-worker'])
			assert.ok(protocol.cargo.includes(name), changed + ' must select ' + name);
		for (const name of [
			'@orchbee/terminal-protocol',
			'@orchbee/terminal-host',
			'@orchbee/terminal',
			'@orchbee/desktop',
			'@orchbee/web',
		])
			assert.ok(protocol.pnpm.includes(name), changed + ' must select ' + name);
	}
	const binding = selectVerificationScope(actual, ['packages/terminal-protocol/src/version.ts']);
	assert.equal(binding.all, false);
	assert.equal(binding.cargo.length, 0);
	assert.ok(!binding.pnpm.includes('@orchbee/terminal-host'));
	const resources = selectVerificationScope(actual, [
		'apps/desktop/resources/agent-observation-adapters/capsule.json',
	]);
	assert.ok(resources.cargo.includes('orchbee'));
	const windowsConsoleHost = selectVerificationScope(actual, [
		'crates/windows-console-host/src/output.rs',
	]);
	assert.equal(windowsConsoleHost.all, false, windowsConsoleHost.reasons.join('\n'));
	assert.ok(windowsConsoleHost.cargo.includes('orchbee-windows-console-host'));
	assert.ok(windowsConsoleHost.cargo.includes('orchbee'));
	const designTokens = selectVerificationScope(actual, ['packages/design-tokens/src/tokens.css']);
	assert.equal(designTokens.all, false, designTokens.reasons.join('\n'));
	assert.ok(designTokens.pnpm.includes('@orchbee/design-tokens'));
	assert.deepEqual(designTokens.cargo, ['orchbee']);
	assert.equal(
		selectVerificationScope(actual, ['packages/design-tokens/src/tokens.css.bak']).cargo.length,
		0,
	);
	const tabCloseCorpus = selectVerificationScope(actual, [
		'packages/tab-strip/src/tab-strip/model/__tests__/fixtures/tab-close-targets.json',
	]);
	assert.equal(tabCloseCorpus.all, false);
	assert.ok(tabCloseCorpus.pnpm.includes('@orchbee/tab-strip'));
	assert.ok(tabCloseCorpus.cargo.includes('orchbee'));
	assert.equal(
		selectVerificationScope(actual, ['packages/tab-strip/src/tab-strip/model/tab-close-targets.ts'])
			.cargo.length,
		0,
	);
	assert.equal(
		selectVerificationScope(actual, [
			'packages/tab-strip/src/tab-strip/model/__tests__/fixtures/tab-close-targets.json.bak',
		]).cargo.length,
		0,
	);
	assert.equal(selectVerificationScope(actual, ['apps/desktop/src/window.ts']).cargo.length, 0);
	assert.ok(
		selectVerificationScope(actual, ['packages/ui/src/component.ts']).pnpm.includes(
			'@orchbee/storybook',
		),
	);
});

test('shared PowerShell startup assets select the Host and its native process consumers', () => {
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	for (const name of ['initialization', 'reader']) {
		const asset = `crates/local-site-runtime/src/windows_powershell_${name}.ps1`;
		const selected = selectVerificationScope(actual, [asset]);
		assert.equal(selected.all, false, selected.reasons.join('\n'));
		assert.ok(selected.pnpm.includes('@orchbee/terminal-host'), asset);
		assert.ok(selected.pnpm.includes('@orchbee/desktop'), asset);
		for (const consumer of ['orchbee', 'orchbee-server', 'orchbee-worker'])
			assert.ok(selected.cargo.includes(consumer), `${asset} must select ${consumer}`);
		assert.equal(
			selectVerificationScope(actual, [asset + '.bak']).pnpm.includes('@orchbee/terminal-host'),
			false,
		);
	}
});

test('paired TUI standards and changed pairing receipts retain native ownership without selecting every suite', () => {
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	const receipt = (digest) => ({ englishSha256: digest.repeat(64), zhCnSha256: 'b'.repeat(64) });
	const before = JSON.stringify({
		formatVersion: 1,
		pairs: { 'docs/tui/terminal-environment.md': receipt('a') },
	});
	const after = JSON.stringify({
		formatVersion: 1,
		pairs: { 'docs/tui/terminal-environment.md': receipt('c') },
	});
	const selected = selectVerificationScope(
		actual,
		[
			'apps/orchbee/src/workbench_client/ui/action_menus.rs',
			'docs/tui/terminal-environment.md',
			'docs/zh-CN/tui/terminal-environment.md',
			'scripts/documentation-pairing.json',
		],
		{ before, after },
	);
	assert.equal(selected.all, false, selected.reasons.join('\n'));
	assert.deepEqual(selected.cargo, ['orchbee']);
	assert.deepEqual(selected.pnpm, []);
	for (const standard of [
		'keyboard-input',
		'layout',
		'mouse-input',
		'overlays-and-status',
		'terminal-environment',
		'verification',
		'visual-foundations',
	]) {
		for (const language of ['', 'zh-CN/']) {
			const result = selectVerificationScope(actual, [`docs/${language}tui/${standard}.md`]);
			assert.equal(result.all, false);
			assert.deepEqual(result.cargo, ['orchbee']);
		}
	}
	for (const versions of [
		{ before, after },
		{ before, after: before, intermediate: [before, after] },
		{ before, after: JSON.stringify({ formatVersion: 1, pairs: {} }) },
		{ before: JSON.stringify({ formatVersion: 1, pairs: {} }), after },
	]) {
		const receiptsOnly = selectVerificationScope(
			actual,
			['scripts/documentation-pairing.json'],
			versions,
		);
		assert.deepEqual(receiptsOnly.cargo, ['orchbee']);
	}
	const reorder = selectVerificationScope(actual, ['scripts/documentation-pairing.json'], {
		before,
		after: JSON.stringify(JSON.parse(before), null, 2),
	});
	assert.deepEqual(reorder.cargo, []);
	assert.equal(reorder.all, false);
	for (const versions of [
		undefined,
		{ before, after: before, intermediate: ['{invalid', before] },
		{ before, after: '{invalid' },
		{ before, after: JSON.stringify({ formatVersion: 2, pairs: {} }) },
		{ before, after: JSON.stringify({ formatVersion: 1, pairs: {}, unexpected: true }) },
		{
			before,
			after: JSON.stringify({ formatVersion: 1, pairs: { 'docs/TESTING.md': receipt('c') } }),
		},
		{
			before,
			after: before,
			intermediate: [
				JSON.stringify({ formatVersion: 1, pairs: { 'docs/TESTING.md': receipt('c') } }),
				before,
			],
		},
		{
			before,
			after: JSON.stringify({ formatVersion: 1, pairs: { 'docs/tui/new.md': receipt('c') } }),
		},
	])
		assert.equal(
			selectVerificationScope(actual, ['scripts/documentation-pairing.json'], versions).all,
			true,
		);
});

test('only declared verification helpers use root tooling evidence; new helpers and shared policies remain conservative', () => {
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	for (const changed of [
		'scripts/run-checks.mjs',
		'scripts/run-rust-workspace.mjs',
		'scripts/run-rust-workspace.test.mjs',
		'scripts/verification-scope.mjs',
		'scripts/verification-scope.test.mjs',
		'scripts/tla/run-models.mjs',
		'scripts/tla/model-cache-cli.test.mjs',
		'scripts/tla/README.zh-CN.md',
		'docs/VERIFICATION.md',
		'docs/zh-CN/VERIFICATION.md',
	]) {
		const selected = selectVerificationScope(actual, [changed]);
		assert.equal(selected.all, false, changed);
		assert.deepEqual(selected.pnpm, []);
		assert.deepEqual(selected.cargo, []);
		assert.equal(checkSteps('test', selected).length, 1);
		assert.deepEqual(
			checkSteps('verify', selected)
				.filter((step) => step.name)
				.map((step) => step.name),
			['format:check', 'test:tla:inventory', 'build:packages', 'lint', 'typecheck', 'test:exports'],
		);
	}
	for (const changed of ['scripts/tla/new-helper.mjs', 'formal/tla/New.tla', 'docs/TESTING.md'])
		assert.equal(selectVerificationScope(actual, [changed]).all, true, changed);
});

test('Git selection covers committed, staged, dirty, untracked, removed and rename paths', (t) => {
	const root = mkdtempSync(path.join(tmpdir(), 'orchbee-verification-scope-'));
	t.after(() => rmSync(root, { recursive: true, force: true }));
	const git = (...args) =>
		execFileSync('git', ['-c', 'core.autocrlf=false', ...args], { cwd: root, encoding: 'utf8' });
	git('init', '--quiet');
	mkdirSync(path.join(root, 'apps/client'), { recursive: true });
	const put = (name, text = 'base\n') => writeFileSync(path.join(root, 'apps/client', name), text);
	for (const name of [
		'committed.rs',
		'staged.rs',
		'masked.rs',
		'dirty.rs',
		'removed.rs',
		'renamed.rs',
	])
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
	put('masked.rs', 'changed\n');
	git('add', '.');
	put('masked.rs');
	put('dirty.rs', 'changed\n');
	put('未跟踪.rs');
	rmSync(path.join(root, 'apps/client/removed.rs'));
	renameSync(path.join(root, 'apps/client/renamed.rs'), path.join(root, 'apps/client/new-name.rs'));
	const expected = [
		'committed.rs',
		'staged.rs',
		'masked.rs',
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

test('affected gate preserves check prerequisites and Node-before-Rust ordering without cross compilation', () => {
	const selected = selectVerificationScope(graph, ['packages/protocol/src/main.ts']);
	const steps = checkSteps('verify', selected);
	assert.deepEqual(
		steps.slice(0, 5).map((step) => step.name),
		['format:check', 'test:tla:inventory', 'build:packages', 'lint', 'typecheck'],
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
	assert.doesNotMatch(JSON.stringify(steps), /test:cross|build:full/u);
	assert.deepEqual(checkSteps('verify', { all: true }), checkSteps('verify'));
	assert.equal(checkSteps('test', selectVerificationScope(graph, [])).length, 1);
});

test('native test prerequisites follow their real consumers and host platform even in scoped or fallback runs', () => {
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	for (const platform of ['win32', 'linux', 'darwin']) {
		for (const [changed, host, peers] of [
			['apps/orchbee/src/main.rs', platform === 'linux', platform === 'linux'],
			['apps/orchbee-worker/src/main.rs', true, platform === 'linux'],
			['apps/desktop/src/main.ts', false, false],
			['scripts/run-checks.mjs', false, false],
			['pnpm-lock.yaml', true, platform === 'linux'],
		]) {
			const scope = selectVerificationScope(actual, [changed]);
			const steps = checkSteps('test', scope, platform);
			const hostIndex = steps.findIndex((step) => step.name === 'prepare native test host');
			const peersIndex = steps.findIndex((step) => step.name === 'prepare native test peers');
			const rustIndex = steps.findIndex((step) => step.name?.endsWith('Rust workspace tests'));
			const nodeIndex = steps.findIndex(
				(step) => step.name?.endsWith('workspace tests') && !step.name.includes('Rust'),
			);
			const label = `${platform}: ${changed}`;
			assert.equal(hostIndex >= 0, host, label);
			assert.equal(peersIndex >= 0, peers, label);
			if (host) assert.ok(nodeIndex < hostIndex && hostIndex < rustIndex, label);
			if (peers) {
				assert.ok(hostIndex < peersIndex && peersIndex < rustIndex, label);
				assert.deepEqual(steps[peersIndex].args, [
					'build',
					'--package',
					'orchbee-server',
					'--package',
					'orchbee-worker',
					'--bins',
					'--all-features',
					'--locked',
				]);
			}
			assert.doesNotMatch(JSON.stringify(steps), /test:cross|build:full|manual:native-window/u);
		}
	}
});
