import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, readFileSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import test from 'node:test';

import {
	changedVerificationPaths,
	readVerificationGraph,
	resolveVerificationScope,
	selectVerificationScope,
	toolingSuiteNames,
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
		'docs/tui/layout.md',
		'docs/tui/verification.md',
		'docs/VERIFICATION.md',
		'README.md',
		'schemas/terminal/README.md',
		'schemas/terminal/references/protocol.md',
		'schemas/keeper/README.md',
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

const stepNames = (steps) =>
	steps.flatMap((step) => (step.name ? [step.name] : step.concurrent.map((member) => member.name)));

test('a root tooling input selects every tooling suite and no workspace suite', () => {
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	for (const changed of [
		'scripts/run-checks.mjs',
		'scripts/verification-scope.test.mjs',
		'scripts/rust-module-graph-record.txt',
		'scripts/release-publish.mjs',
		'.github/workflows/verify.yml',
		'.github/workflows/release.yml',
		'.github/actions/zig/action.yml',
		'knip.config.mjs',
	]) {
		const selected = selectVerificationScope(actual, [changed]);
		assert.equal(selected.all, false, changed);
		assert.equal(selected.rootTooling, true, changed);
		assert.deepEqual([...selected.pnpm, ...selected.cargo], [], changed);
		assert.deepEqual(selected.tooling, toolingSuiteNames, changed);
		const names = stepNames(checkSteps('verify', selected));
		for (const suite of ['test:check-output', 'test:release-feed', 'test:exports'])
			assert.ok(names.includes(suite), `${changed} must run ${suite}`);
		assert.doesNotMatch(names.join('\n'), /workspace tests|TLA/u, changed);
	}
	// A root pnpm manifest is a tooling input and reaches every pnpm workspace as well.
	const manifest = selectVerificationScope(actual, ['package.json']);
	assert.deepEqual(manifest.tooling, toolingSuiteNames);
	assert.ok(manifest.pnpm.includes('@orchbee/web'));
	// A file a single suite reads is not a root tooling input.
	assert.equal(selectVerificationScope(actual, ['packages/ui/src/lib/cn.ts']).rootTooling, false);
	// Anything else outside an owner stays conservative.
	for (const changed of ['.github/dependabot.yml', 'tsconfig.base.json'])
		assert.equal(selectVerificationScope(actual, [changed]).all, true, changed);
});

test('only a changed TLA+ model or TLA+ tool runs TLA+, and it selects no suite', () => {
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	for (const changed of [
		'apps/terminal-host/formal/tla/TerminalHostHandover.tla',
		'crates/local-runtime/formal/tla/deep/ReleaseStoreFallback.cfg',
		'scripts/tla/models.json',
	]) {
		const selected = selectVerificationScope(actual, [changed]);
		assert.equal(selected.all, false, changed);
		assert.equal(selected.tla, true, changed);
		assert.deepEqual([...selected.pnpm, ...selected.cargo, ...selected.tooling], [], changed);
		assert.ok(stepNames(checkSteps('verify', selected)).includes('changed TLA+ models'), changed);
	}
	for (const scope of [
		selectVerificationScope(actual, ['apps/terminal-host/src/main.zig']),
		{ all: true, tla: false, reasons: [] },
	])
		assert.doesNotMatch(stepNames(checkSteps('verify', scope)).join('\n'), /TLA/u);
	assert.doesNotMatch(stepNames(checkSteps('verify')).join('\n'), /TLA/u);
});

test('a tooling suite runs with the repository files it reads, and none runs for other changes', () => {
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	for (const [changed, suite, owner] of [
		['apps/website/src/website/release.ts', 'test:release-feed', '@orchbee/website'],
		['apps/orchbee/src/cli/update/signature.rs', 'test:release-feed', 'orchbee'],
		['packages/design-tokens/src/tokens.css', 'test:style-contract', '@orchbee/design-tokens'],
		[
			'crates/agent-integration-writer/agent-observation-adapters/capsule.json',
			'test:agent-observation-adapters',
			'orchbee-agent-integration-writer',
		],
	]) {
		const selected = selectVerificationScope(actual, [changed]);
		assert.equal(selected.all, false, selected.reasons.join('\n'));
		assert.deepEqual(selected.tooling, [suite], changed);
		assert.ok([...selected.pnpm, ...selected.cargo].includes(owner), changed);
	}
	const registry = selectVerificationScope(actual, ['schemas/connection/v1/connections.json']);
	assert.equal(registry.all, false, registry.reasons.join('\n'));
	assert.deepEqual(registry.tooling, ['test:release-feed']);
	assert.ok(registry.pnpm.includes('@orchbee/terminal-host'));
	assert.ok(registry.cargo.includes('orchbee-connection-handshake'));
	for (const changed of ['docs/TESTING.md', 'apps/web/src/main.tsx', 'apps/orchbee/src/main.rs']) {
		const selected = selectVerificationScope(actual, [changed]);
		assert.deepEqual(selected.tooling, [], changed);
		const names = stepNames(checkSteps('verify', selected));
		// Markdown links cover every tracked file, so that suite runs for any change.
		assert.ok(names.includes('test:lint-rules'), changed);
		assert.ok(!names.includes('test:check-output'), changed);
		assert.doesNotMatch(names.join('\n'), /TLA/u, changed);
	}
});

test('the changed gate runs Clippy and rustfmt only for the affected Cargo owners', () => {
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	const rustSteps = (changed) => {
		const selected = selectVerificationScope(actual, [changed]);
		assert.equal(selected.all, false, selected.reasons.join('\n'));
		const steps = checkSteps('verify', selected).filter(
			(step) =>
				step.name === 'lint:rust' ||
				(step.args?.includes('scripts/run-rust-workspace.mjs') && !step.args.includes('test')),
		);
		return { selected, steps };
	};
	for (const changed of ['docs/TESTING.md', 'apps/web/src/main.tsx', 'packages/ui/src/x.ts'])
		assert.deepEqual(rustSteps(changed).steps, [], changed);
	const { selected, steps } = rustSteps('crates/terminal-protocol/src/lib.rs');
	assert.deepEqual(
		steps.map((step) => step.args.slice(-1)[0] === 'format:check'),
		[true, false],
	);
	assert.deepEqual(steps[1].args, [
		'scripts/run-rust-workspace.mjs',
		'lint',
		...selected.cargo.flatMap((name) => ['--package', name]),
	]);
	assert.ok(selected.cargo.length > 1 && selected.unaffected.length > 0);
	// The runner holds the workspace's lint policies, so its change lints every member.
	assert.deepEqual(
		rustSteps('scripts/run-rust-workspace.mjs').steps.map((step) => step.name),
		['lint:rust'],
	);
	// Every other command of the complete lint script runs for any change.
	const scripts = JSON.parse(
		readFileSync(path.resolve(import.meta.dirname, '../package.json'), 'utf8'),
	).scripts;
	const always = scripts.lint
		.split(' && ')
		.map((part) => part.replace(/^pnpm /u, ''))
		.filter((name) => name !== 'lint:rust');
	const names = stepNames(checkSteps('verify', selectVerificationScope(actual, ['README.md'])));
	for (const name of always) assert.ok(names.includes(name), name);
});

test('a Terminal Host source change compiles its tests for the two POSIX release systems', () => {
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	const compilation = (changed) =>
		checkSteps('verify', selectVerificationScope(actual, [changed]), 'win32').filter((step) =>
			step.args?.includes('test-compile'),
		);
	for (const changed of ['apps/terminal-host/src/main.zig', 'apps/terminal-host/build.zig']) {
		const steps = compilation(changed);
		assert.deepEqual(
			steps.map((step) => step.args.at(-1)),
			['-Dtarget=x86_64-linux-gnu', '-Dtarget=aarch64-macos'],
			changed,
		);
		for (const step of steps) assert.ok(step.args.includes('@orchbee/terminal-host'), changed);
	}
	for (const changed of ['apps/terminal-host/README.md', 'apps/orchbee/src/main.rs'])
		assert.deepEqual(compilation(changed), [], changed);
});

test('the export check runs for a library package or a tooling change only', () => {
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	for (const [changed, expected] of [
		['packages/ui/src/component.ts', true],
		['scripts/check-package-exports.mjs', true],
		['apps/web/src/main.tsx', false],
		['apps/orchbee/src/main.rs', false],
		['docs/PACKAGES.md', false],
	]) {
		const names = stepNames(checkSteps('verify', selectVerificationScope(actual, [changed])));
		assert.equal(names.includes('test:exports'), expected, changed);
	}
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
	const fields = ({ verbose, plan, local, changedSince }) => ({
		verbose,
		plan,
		local,
		changedSince,
	});
	assert.deepEqual(fields(verificationOptions(['--verbose'])), {
		verbose: true,
		plan: false,
		local: false,
		changedSince: undefined,
	});
	assert.deepEqual(fields(verificationOptions(['--changed-since', 'main', '--', '--plan'])), {
		verbose: false,
		plan: true,
		local: false,
		changedSince: 'main',
	});
	assert.equal(verificationOptions(['--local', '--changed-since', '--', 'main']).local, true);
	for (const args of [
		['--changed-since'],
		['--changed-since', '--plan'],
		['--plan'],
		['--local'],
		['--all'],
		['--changed-since', 'main', '--changed-since', 'HEAD'],
	])
		assert.throws(() => verificationOptions(args));
});

test('affected gate preserves check prerequisites and Node-before-Rust ordering without cross compilation', () => {
	const selected = selectVerificationScope(graph, ['packages/protocol/src/main.ts']);
	const steps = checkSteps('verify', selected);
	// Dependency lint and type checks read the packages' build output.
	const position = (name) => steps.findIndex((step) => step.name === name);
	assert.ok(position('build:packages') >= 0);
	for (const dependent of ['lint:dependencies', 'typecheck'])
		assert.ok(position('build:packages') < position(dependent), dependent);
	assert.ok(position('typecheck') < position('affected workspace tests'));
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

test('the local gate tests the changed owners only and keeps Clippy over their consumers', () => {
	const selected = selectVerificationScope(graph, ['crates/protocol/src/lib.rs']);
	assert.deepEqual(selected.owners, { pnpm: [], cargo: ['rust-protocol'] });
	assert.deepEqual(selected.cargo, ['client', 'rust-protocol', 'server', 'worker']);
	const steps = checkSteps('verify:local', selected, 'win32');
	const step = (name) => steps.find((entry) => entry.name === name);
	assert.deepEqual(step('changed Rust tests').args, [
		'scripts/run-rust-workspace.mjs',
		'test',
		'--local',
		'--package',
		'rust-protocol',
	]);
	assert.ok(step('affected Rust lint').args.includes('client'));
	assert.equal(step('affected Rust workspace tests'), undefined);
	// A foreign consumer joins the plan, not the owners whose suites the local gate runs.
	const actual = readVerificationGraph(path.resolve(import.meta.dirname, '..'));
	const host = selectVerificationScope(actual, [
		'apps/terminal-host/src/core/attachment_projection.zig',
	]);
	assert.deepEqual(host.owners, { pnpm: ['@orchbee/terminal-host'], cargo: [] });
	assert.ok(host.cargo.length > 0);
	assert.deepEqual(
		checkSteps('verify:local', { all: true }, 'win32'),
		checkSteps('verify', undefined, 'win32'),
	);
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
