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

test('unknown/global paths and unsupported graphs select all rather than omit evidence', () => {
	for (const file of ['docs/diagram.svg', 'LICENSE', 'nul', 'apps/retired/src/main.rs'])
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
		[
			...graph,
			{ root: 'third-party/nested/crate', name: 'nested', kind: 'vendored', dependencies: [] },
		],
	])
		assert.equal(selectVerificationScope(invalid, []).all, true);
});

test("a manifest change selects its package manager's owners and their consumer closure", () => {
	const cargo = selectVerificationScope(graph, ['Cargo.lock']);
	assert.equal(cargo.all, false, cargo.reasons.join('\n'));
	assert.deepEqual(cargo.cargo, ['client', 'rust-protocol', 'server', 'worker']);
	assert.deepEqual(cargo.pnpm, []);
	const pnpm = selectVerificationScope(graph, ['packages/protocol/package.json']);
	assert.equal(pnpm.all, false, pnpm.reasons.join('\n'));
	assert.deepEqual(pnpm.pnpm, ['@test/independent', '@test/protocol', '@test/web']);
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	const lockfile = selectVerificationScope(actual, ['pnpm-lock.yaml']);
	assert.equal(lockfile.all, false, lockfile.reasons.join('\n'));
	// The Host is a pnpm owner; its native process consumers follow the reviewed foreign edges.
	for (const name of ['orchbee', 'orchbee-server', 'orchbee-worker'])
		assert.ok(lockfile.cargo.includes(name), `pnpm-lock.yaml must select ${name}`);
	const bundle = selectVerificationScope(actual, [
		'crates/agent-integration-writer/agent-observation-adapters/bundle/package.json',
	]);
	assert.equal(bundle.all, false, bundle.reasons.join('\n'));
	assert.ok(bundle.cargo.includes('orchbee-agent-integration-writer'), 'own owner');
});

test('documentation without a test consumer selects no suite', () => {
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	for (const changed of [
		'proposals/example.md',
		'proposals/example/diagram.png',
		'docs/TESTING.md',
		'docs/frontend/react.md',
		'README.md',
	]) {
		const selected = selectVerificationScope(actual, [changed]);
		assert.equal(selected.all, false, changed);
		assert.deepEqual([...selected.pnpm, ...selected.cargo], [], changed);
	}
	for (const changed of ['docs/diagram.svg', 'docs/tui/assets/frame.txt'])
		assert.equal(selectVerificationScope(actual, [changed]).all, true, changed);
});

test('internal Cargo source selects its exact owner and dependent siblings before the enclosing application', () => {
	const internal = [
		{
			root: 'apps/client',
			name: 'client',
			kind: 'cargo',
			dependencies: ['apps/client/crates/view'],
		},
		{ root: 'apps/client/crates/io', name: 'client-io', kind: 'cargo', dependencies: [] },
		{
			root: 'apps/client/crates/view',
			name: 'client-view',
			kind: 'cargo',
			dependencies: ['apps/client/crates/io'],
		},
		{
			root: 'apps/client/crates/independent',
			name: 'client-independent',
			kind: 'cargo',
			dependencies: [],
		},
	];
	for (const ordering of [internal, internal.toReversed()]) {
		const selected = selectVerificationScope(ordering, ['apps/client/crates/io/src/lib.rs']);
		assert.equal(selected.all, false, selected.reasons.join('\n'));
		assert.deepEqual(selected.cargo, ['client', 'client-io', 'client-view']);
		assert.deepEqual(selected.unaffected, ['apps/client/crates/independent']);
		assert.deepEqual(selectVerificationScope(ordering, ['apps/client/src/main.rs']).cargo, [
			'client',
		]);
	}
	assert.equal(selectVerificationScope(internal.slice(1), []).all, true);
	assert.equal(
		selectVerificationScope(
			internal.map((entry, index) => (index === 1 ? { ...entry, kind: 'pnpm' } : entry)),
			[],
		).all,
		true,
	);
});

test('real workspace selection retains foreign fixture, resource, process and discovery consumers', () => {
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	const host = selectVerificationScope(actual, ['apps/terminal-host/src/main.zig']);
	assert.equal(host.all, false, host.reasons.join('\n'));
	for (const name of ['orchbee', 'orchbee-server', 'orchbee-worker'])
		assert.ok(host.cargo.includes(name), name);
	for (const [provider, consumers] of [
		['apps/terminal-host', ['apps/orchbee', 'apps/orchbee-server', 'apps/orchbee-worker']],
		['apps/orchbee-worker', ['apps/orchbee', 'apps/orchbee-server']],
		['apps/orchbee-server', ['apps/orchbee']],
		['apps/web', ['apps/desktop', 'apps/storybook']],
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
		for (const name of [
			'orchbee-terminal-protocol',
			'orchbee-connection-handshake',
			'orchbee',
			'orchbee-server',
			'orchbee-worker',
		])
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
		'crates/agent-integration-writer/agent-observation-adapters/capsule.json',
	]);
	assert.equal(resources.all, false, resources.reasons.join('\n'));
	assert.ok(resources.cargo.includes('orchbee-agent-integration-writer'));
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
	// The native theme reads the shared theme colors, not the CSS registry.
	assert.equal(designTokens.cargo.length, 0);
	for (const generated of [
		'packages/design-tokens/src/color-themes.generated.css',
		'packages/terminal/src/color-themes/color-themes.generated.ts',
	]) {
		const selected = selectVerificationScope(actual, [generated]);
		assert.equal(selected.all, false, selected.reasons.join('\n'));
		assert.ok(
			selected.cargo.includes('orchbee-color-themes'),
			generated + ' must select its generator',
		);
	}
	for (const [contract, packages] of [
		[
			'schemas/workbench/v1/workbench-action-identities-v1.json',
			['@orchbee/tab-strip', '@orchbee/web'],
		],
		['schemas/workbench/v1/tab-close-targets-conformance-v1.json', ['@orchbee/tab-strip']],
		['schemas/workbench/v1/shared-theme-colors-v1.json', ['@orchbee/design-tokens']],
	]) {
		const parity = selectVerificationScope(actual, [contract]);
		assert.equal(parity.all, false, parity.reasons.join('\n'));
		for (const name of packages)
			assert.ok(parity.pnpm.includes(name), `${contract} must select ${name}`);
		assert.ok(parity.cargo.includes('orchbee-workbench-client'), contract);
		assert.equal(selectVerificationScope(actual, [contract + '.bak']).all, true);
	}
	assert.equal(
		selectVerificationScope(actual, ['packages/tab-strip/src/tab-strip/model/tab-close-targets.ts'])
			.cargo.length,
		0,
	);
	assert.equal(selectVerificationScope(actual, ['apps/desktop/src/window.ts']).cargo.length, 0);
	assert.ok(
		selectVerificationScope(actual, ['packages/ui/src/component.ts']).pnpm.includes(
			'@orchbee/storybook',
		),
	);
});

test('a vendored Cargo package selects the members that resolve it rather than every suite', () => {
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	for (const changed of [
		'third-party/process-wrap/src/windows.rs',
		'third-party/process-wrap/README.md',
	]) {
		const selected = selectVerificationScope(actual, [changed]);
		assert.equal(selected.all, false, selected.reasons.join('\n'));
		for (const name of [
			'orchbee',
			'orchbee-agent-integration-writer',
			'orchbee-launcher',
			'orchbee-local-runtime',
			'orchbee-server',
			'orchbee-workbench-client',
			'orchbee-worker',
		])
			assert.ok(selected.cargo.includes(name), `${changed} must select ${name}`);
		assert.ok(!selected.cargo.includes('orchbee-terminal-protocol'), changed);
	}
	// The vendored owner has no suite of its own, so it is never reported as unexecuted.
	assert.ok(
		!selectVerificationScope(actual, ['packages/ui/src/component.ts']).unaffected.includes(
			'third-party/process-wrap',
		),
	);
	for (const changed of ['third-party/README.md', 'third-party/unvendored/src/lib.rs'])
		assert.equal(selectVerificationScope(actual, [changed]).all, true, changed);
});

test('shared PowerShell startup assets select the Host and its native process consumers', () => {
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	for (const name of ['initialization', 'reader']) {
		const asset = `crates/local-runtime/src/directory/windows/${name}.ps1`;
		const selected = selectVerificationScope(actual, [asset]);
		assert.equal(selected.all, false, selected.reasons.join('\n'));
		assert.ok(selected.pnpm.includes('@orchbee/terminal-host'), asset);
		for (const consumer of ['orchbee', 'orchbee-server', 'orchbee-worker'])
			assert.ok(selected.cargo.includes(consumer), `${asset} must select ${consumer}`);
		// Only the exact asset is a Host input; a sibling still reaches the Host's ConPTY witness
		// through orchbee, so the probe is the foreign-input reason rather than the selection.
		assert.equal(
			selectVerificationScope(actual, [asset + '.bak']).reasons.includes(
				`apps/terminal-host <- ${asset}.bak`,
			),
			false,
		);
	}
});

test('TUI standards retain native ownership without selecting every suite', () => {
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	const selected = selectVerificationScope(actual, [
		'apps/orchbee/crates/workbench-client/src/workbench_client/ui/action_menus.rs',
		'docs/tui/terminal-environment.md',
	]);
	assert.equal(selected.all, false, selected.reasons.join('\n'));
	assert.deepEqual(selected.cargo, ['orchbee', 'orchbee-workbench-client']);
	// The Host's suite runs the orchbee binary's ConPTY witness, so it joins without its consumers.
	assert.deepEqual(selected.pnpm, ['@orchbee/terminal-host']);
	for (const standard of [
		'keyboard-input',
		'layout',
		'mouse-input',
		'overlays-and-status',
		'terminal-environment',
		'verification',
		'visual-foundations',
	]) {
		const result = selectVerificationScope(actual, [`docs/tui/${standard}.md`]);
		assert.equal(result.all, false);
		assert.deepEqual(result.cargo, ['orchbee']);
	}
});

test('only declared verification helpers use root tooling evidence; new helpers remain conservative', () => {
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	for (const changed of [
		'scripts/run-checks.mjs',
		'scripts/run-rust-workspace.mjs',
		'scripts/run-rust-workspace.test.mjs',
		'scripts/verification-scope.mjs',
		'scripts/verification-scope.test.mjs',
		'scripts/rust-module-graph.mjs',
		'scripts/rust-module-graph-record.txt',
		'scripts/tla/run-models.mjs',
		'scripts/tla/model-cache-cli.test.mjs',
		'docs/VERIFICATION.md',
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
	for (const changed of ['scripts/tla/new-helper.mjs', 'formal/tla/New.tla'])
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
	// A base that does not bound the change never yields a plan: it is refused, not widened.
	const sibling = git(
		'-c',
		'user.name=Fixture',
		'-c',
		'user.email=fixture@example.invalid',
		'commit-tree',
		'HEAD^{tree}',
		'-p',
		base,
		'-m',
		'sibling',
	).trim();
	for (const [reference, problem] of [
		['no-such-base', /no-such-base does not resolve.*git merge-base no-such-base HEAD/su],
		[sibling, /is not an ancestor of HEAD\. Rebase onto/u],
	])
		assert.throws(() => resolveVerificationScope(root, reference), problem);
	git('update-index', '--assume-unchanged', 'apps/client/dirty.rs');
	assert.throws(() => changedVerificationPaths(root, base), /assume-unchanged/u);
	assert.equal(resolveVerificationScope(root, base).all, true);
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
		const cliCohort = platform !== 'darwin';
		for (const [changed, host, peers] of [
			['apps/orchbee/src/main.rs', cliCohort, cliCohort],
			['apps/orchbee-worker/src/main.rs', true, cliCohort],
			['apps/desktop/src/main.ts', false, false],
			['scripts/run-checks.mjs', false, false],
			['nul', true, cliCohort],
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
			assert.doesNotMatch(JSON.stringify(steps), /test:cross|build:full/u);
		}
	}
});
