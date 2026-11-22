import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';

const packageName = /^(?:@[a-z\d._-]+\/)?[a-z\d][a-z\d._-]*$/u;
const pnpmManifestName = /(?:^|\/)(?:package\.json|pnpm-lock\.yaml|pnpm-workspace\.yaml)$/u;
const cargoManifestName = /(?:^|\/)(?:Cargo\.toml|Cargo\.lock)$/u;
const workspaceRoot =
	/^(?:(?:apps|packages|crates)\/[a-z\d-]+|apps\/[a-z\d-]+\/crates\/[a-z\d-]+)$/u;
const vendoredRoot = /^third-party\/[a-z\d-]+$/u;

// Runtime/process inputs not represented by package managers. The root verification owner
// maintains these edges with their consumers; a new foreign input needs a positive control.
const foreignDependencies = [
	['apps/orchbee', 'apps/orchbee-server'],
	['apps/orchbee', 'apps/orchbee-worker'],
	['apps/orchbee', 'apps/terminal-host'],
	['apps/orchbee-server', 'apps/orchbee-worker'],
	['apps/orchbee-server', 'apps/terminal-host'],
	['apps/orchbee-worker', 'apps/terminal-host'],
	['apps/desktop', 'apps/web'],
];
// Suites that run a provider's built artifact while their own artifact stays unchanged. They join
// the plan after consumer closure and select no consumers of their own; a dependency edge would
// close a cycle with the provider's own edge to them.
const testWitnesses = [
	// On Windows x64 the Host's test step builds and runs the orchbee binary's ConPTY pointer witness.
	['apps/terminal-host', 'apps/orchbee'],
];
const foreignPaths = [
	['crates/local-runtime/src/directory/windows/initialization.ps1', 'apps/terminal-host'],
	['crates/local-runtime/src/directory/windows/reader.ps1', 'apps/terminal-host'],
	['crates/local-runtime/src/directory/windows/start.ps1', 'apps/terminal-host'],
	// The theme crate's staleness test reads the files it generates.
	['packages/design-tokens/src/color-themes.generated.css', 'crates/color-themes'],
	['packages/terminal/src/color-themes/color-themes.generated.ts', 'crates/color-themes'],
];
// Language-neutral parity sources with exact consumers; other shared schema inputs keep the
// full gate until their consumers are recorded here.
const sharedContractInputs = [
	[
		// The Host embeds the registry and both sides hash it into the development fingerprint.
		'schemas/connection/v1/connections.json',
		['apps/terminal-host', 'crates/connection-handshake'],
	],
	[
		'schemas/workbench/v1/workbench-action-identities-v1.json',
		['apps/web', 'packages/tab-strip', 'apps/orchbee/crates/workbench-client'],
	],
	[
		'schemas/workbench/v1/tab-close-targets-conformance-v1.json',
		['packages/tab-strip', 'apps/orchbee/crates/workbench-client'],
	],
	[
		'schemas/workbench/v1/shared-theme-colors-v1.json',
		['packages/design-tokens', 'apps/orchbee/crates/workbench-client'],
	],
];

// The root tooling suites test the gate's own tools, mostly on fixtures. A root tooling input, which
// no workspace owns, selects all of them and no workspace suite. A suite also runs when a
// repository file it reads changes (an input ending in `/` is a directory); that file keeps its
// own owner as well.
const rootToolingManifests = new Set([
	'package.json',
	'pnpm-lock.yaml',
	'pnpm-workspace.yaml',
	'knip.config.mjs',
]);
const workflowFile = /^\.github\/workflows\/[^/]+\.ya?ml$/u;
const toolingSuites = [
	['test:check-output', []],
	['test:rust-runner', []],
	['test:rust-modules', []],
	['test:dependency-scope', ['apps/storybook/package.json']],
	['test:ghostty-source', []],
	['test:style-contract', ['packages/design-tokens/', 'packages/ui/']],
	['test:storage-contract', []],
	['test:stage-local-runtime', ['apps/terminal-host/scripts/windows-conpty.mjs']],
	[
		'test:release-feed',
		[
			'schemas/connection/v1/connections.json',
			'apps/orchbee/npm/',
			'apps/orchbee/src/cli/update/signature.rs',
			'apps/orchbee/src/cli/update/tests/fixtures/',
			'apps/website/src/website/release.ts',
		],
	],
	['test:agent-observation-adapters', ['crates/agent-integration-writer/']],
];
// A TLA+ model or its tooling selects no suite: the changed gate runs the changed models instead.
const tlaInput = (changed) =>
	changed.startsWith('scripts/tla/') || /(?:^|\/)formal\/tla\//u.test(changed);
// The complete Rust lint runs for a change to the runner that holds its policies.
const rustLintTool = 'scripts/run-rust-workspace.mjs';
// Zig sources that the Windows host suite compiles for Windows only.
const posixCompileInputs = [
	'apps/terminal-host/src/',
	'apps/terminal-host/build/',
	'apps/terminal-host/build.zig',
	'apps/terminal-host/build.zig.zon',
];
const matchesInput = (changed, input) =>
	input.endsWith('/') ? changed.startsWith(input) : changed === input;

export const toolingSuiteNames = Object.freeze(toolingSuites.map(([name]) => name));
function capture(root, command, args, stdio) {
	return execFileSync(command, args, {
		cwd: root,
		encoding: 'utf8',
		maxBuffer: 16 * 1024 ** 2,
		...(stdio === undefined ? {} : { stdio }),
	});
}

// A base that does not bound the change is refused rather than widened: the caller must choose
// between rebasing and checking the branch from its fork point.
class UnboundedBaseError extends Error {}

function refuseBase(reference, problem) {
	return new UnboundedBaseError(
		`Base ${reference} ${problem}. Rebase onto ${reference}, or pass its fork point ` +
			`(git merge-base ${reference} HEAD) to check the branch as it stands.`,
	);
}

export function changedVerificationPaths(root, reference) {
	let base;
	try {
		base = capture(
			root,
			'git',
			['rev-parse', '--verify', '--end-of-options', `${reference}^{commit}`],
			['ignore', 'pipe', 'pipe'],
		).trim();
	} catch {
		throw refuseBase(reference, 'does not resolve to a commit');
	}
	try {
		capture(root, 'git', ['merge-base', '--is-ancestor', base, 'HEAD'], ['ignore', 'pipe', 'pipe']);
	} catch (error) {
		// Exit status 1 is Git's answer "not an ancestor"; any other failure is a Git error.
		if (error.status === 1) throw refuseBase(reference, 'is not an ancestor of HEAD');
		throw error;
	}
	const hidden = capture(root, 'git', ['ls-files', '-v', '-z']).split('\0');
	if (hidden.some((entry) => entry[0] === 'S' || (entry && entry[0] === entry[0].toLowerCase())))
		throw new Error('Git skip-worktree or assume-unchanged inputs prevent affected verification');
	const paths = new Set([
		...capture(root, 'git', [
			'diff',
			'--no-renames',
			'--name-only',
			'-z',
			base,
			'HEAD',
			'--',
		]).split('\0'),
		...capture(root, 'git', [
			'diff',
			'--cached',
			'--no-renames',
			'--name-only',
			'-z',
			'HEAD',
			'--',
		]).split('\0'),
		...capture(root, 'git', ['diff', '--no-renames', '--name-only', '-z', '--']).split('\0'),
		...capture(root, 'git', ['ls-files', '--others', '--exclude-standard', '-z']).split('\0'),
	]);
	paths.delete('');
	return { base, paths: [...paths].toSorted() };
}

export function readVerificationGraph(root) {
	const graph = [];
	for (const parent of ['apps', 'packages']) {
		for (const entry of readdirSync(path.join(root, parent), { withFileTypes: true })) {
			if (!entry.isDirectory()) continue;
			const owner = `${parent}/${entry.name}`;
			let manifest;
			try {
				manifest = JSON.parse(readFileSync(path.join(root, owner, 'package.json'), 'utf8'));
			} catch (error) {
				if (error.code === 'ENOENT') continue;
				throw error;
			}
			graph.push({ root: owner, name: manifest.name, kind: 'pnpm', dependencies: [], manifest });
		}
	}
	const pnpm = new Map(graph.map((entry) => [entry.name, entry]));
	if (pnpm.size !== graph.length) throw new Error('Duplicate pnpm workspace names');
	for (const entry of graph) {
		for (const kind of [
			'dependencies',
			'devDependencies',
			'optionalDependencies',
			'peerDependencies',
		]) {
			for (const [name, version] of Object.entries(entry.manifest[kind] ?? {})) {
				if (pnpm.has(name)) entry.dependencies.push(pnpm.get(name).root);
				else if (/^(?:workspace:|link:|file:)/u.test(version))
					throw new Error(`Unmapped local dependency ${entry.name}: ${name}`);
			}
		}
		delete entry.manifest;
	}
	graph.push(...readCargoVerificationGraph(root));
	for (const [consumer, provider] of foreignDependencies) {
		const owner = graph.find((entry) => entry.root === consumer);
		if (!owner || !graph.some((entry) => entry.root === provider))
			throw new Error(`Unmapped foreign dependency ${consumer} <- ${provider}`);
		owner.dependencies.push(provider);
	}
	const storybook = graph.find((entry) => entry.root === 'apps/storybook');
	if (!storybook) throw new Error('Missing Storybook discovery owner');
	storybook.dependencies.push(
		...graph
			.filter((entry) => entry.root.startsWith('packages/') || entry.root === 'apps/web')
			.map((entry) => entry.root),
	);
	return graph;
}

export function readCargoVerificationGraph(root) {
	const graph = [];
	const metadata = JSON.parse(
		capture(root, 'cargo', [
			'metadata',
			'--format-version',
			'1',
			'--all-features',
			'--locked',
			'--quiet',
		]),
	);
	const ownerRoot = (entry) =>
		path.relative(root, path.dirname(entry.manifest_path)).replaceAll('\\', '/');
	const members = metadata.packages.filter((entry) =>
		metadata.workspace_members.includes(entry.id),
	);
	// A path package that Cargo resolves from third-party/ for a member stays outside the workspace
	// and has no suite of its own, but it owns its directory: consumer closure carries a change
	// there to every member that resolves it.
	const vendored = metadata.packages.filter(
		(entry) =>
			entry.source === null &&
			!metadata.workspace_members.includes(entry.id) &&
			ownerRoot(entry).startsWith('third-party/'),
	);
	const owners = [...members, ...vendored];
	const byPath = new Map(
		owners.map((entry) => [path.resolve(path.dirname(entry.manifest_path)), entry]),
	);
	const byId = new Map(owners.map((entry) => [entry.id, entry]));
	const resolved = new Map((metadata.resolve?.nodes ?? []).map((entry) => [entry.id, entry.deps]));
	for (const entry of owners) {
		const owner = ownerRoot(entry);
		for (const dependency of entry.dependencies) {
			if (dependency.path !== undefined && !byPath.has(path.resolve(dependency.path)))
				throw new Error(`Unmapped local Cargo dependency ${entry.name}: ${dependency.path}`);
		}
		const dependencies = new Set();
		const visited = new Set();
		function visit(id) {
			if (visited.has(id)) return;
			visited.add(id);
			const node = resolved.get(id);
			if (!node) throw new Error(`Missing resolved Cargo dependency node ${id}`);
			for (const dependency of node) {
				const target = byId.get(dependency.pkg);
				if (target) dependencies.add(ownerRoot(target));
				else visit(dependency.pkg);
			}
		}
		// Follow actual package IDs through patched declarations and nonmember dependencies;
		// each owner boundary owns the next step of consumer closure.
		visit(entry.id);
		graph.push({
			root: owner,
			name: entry.name,
			kind: vendored.includes(entry) ? 'vendored' : 'cargo',
			dependencies: [...dependencies],
		});
	}
	return graph;
}

function validateGraph(graph) {
	const roots = new Set(graph.map((entry) => entry.root));
	const names = new Set(graph.map((entry) => `${entry.kind}:${entry.name}`));
	if (roots.size !== graph.length || names.size !== graph.length)
		throw new Error('Ambiguous workspace owner');
	for (const entry of graph) {
		if (
			!packageName.test(entry.name) ||
			!(entry.kind === 'vendored' ? vendoredRoot : workspaceRoot).test(entry.root) ||
			!['pnpm', 'cargo', 'vendored'].includes(entry.kind) ||
			!Array.isArray(entry.dependencies) ||
			entry.dependencies.some((dependency) => !roots.has(dependency))
		)
			throw new Error(`Unsupported workspace owner ${entry.root}`);
		const application = /^(apps\/[a-z\d-]+)\/crates\/[a-z\d-]+$/u.exec(entry.root)?.[1];
		if (application && (entry.kind !== 'cargo' || !roots.has(application)))
			throw new Error(`Missing application owner for internal crate ${entry.root}`);
	}
	const complete = new Set();
	function visit(root, active) {
		if (active.has(root)) throw new Error(`Cyclic verification dependencies at ${root}`);
		if (complete.has(root)) return;
		const next = new Set([...active, root]);
		for (const dependency of graph.find((entry) => entry.root === root).dependencies)
			visit(dependency, next);
		complete.add(root);
	}
	for (const entry of graph) visit(entry.root, new Set());
}

export function selectVerificationScope(graph, paths) {
	const tla = paths.some(tlaInput);
	try {
		validateGraph(graph);
		const selected = new Set();
		const tooling = new Set();
		const reasons = [];
		let completeRustLint = false;
		let posixCompile = false;
		// Nested Cargo members own their source before the enclosing application's prefix.
		const owners = graph.toSorted((left, right) => right.root.length - left.root.length);
		for (const changed of paths) {
			if (changed === rustLintTool) completeRustLint = true;
			if (posixCompileInputs.some((input) => matchesInput(changed, input))) posixCompile = true;
			if (tlaInput(changed)) {
				reasons.push(`TLA+ <- ${changed}`);
				continue;
			}
			for (const [suite, inputs] of toolingSuites) {
				if (inputs.some((input) => matchesInput(changed, input))) tooling.add(suite);
			}
			if (
				changed.startsWith('scripts/') ||
				workflowFile.test(changed) ||
				rootToolingManifests.has(changed)
			) {
				for (const [suite] of toolingSuites) tooling.add(suite);
				reasons.push(`root tooling <- ${changed}`);
				// A root pnpm manifest also reaches every pnpm workspace below.
				if (!pnpmManifestName.test(changed)) continue;
			}
			const manager = [
				['pnpm', pnpmManifestName],
				['cargo', cargoManifestName],
			].find(([, name]) => name.test(changed))?.[0];
			if (manager !== undefined) {
				// A manifest or lockfile change reaches no further than every owner of its package
				// manager; consumer closure below carries it across reviewed foreign edges.
				for (const entry of graph) if (entry.kind === manager) selected.add(entry.root);
				reasons.push(`every ${manager === 'pnpm' ? 'pnpm' : 'Cargo'} workspace <- ${changed}`);
				const owner = owners.find((entry) => changed.startsWith(`${entry.root}/`));
				if (owner) {
					selected.add(owner.root);
					reasons.push(`${owner.root} <- ${changed}`);
				}
				continue;
			}
			// Proposals are never current authority, and repository standards, root Markdown and the
			// schema directories' Markdown have no test reader; Markdown inside an owner stays with
			// that owner.
			if (
				changed.startsWith('proposals/') ||
				(changed.endsWith('.md') &&
					(changed.startsWith('docs/') || changed.startsWith('schemas/') || !changed.includes('/')))
			) {
				reasons.push(`documentation <- ${changed}`);
				continue;
			}
			if (changed.startsWith('schemas/terminal/')) {
				// The connection handshake's registry test checks the frozen terminal fixtures.
				for (const root of [
					'crates/terminal-protocol',
					'packages/terminal-protocol',
					'apps/terminal-host',
					'crates/connection-handshake',
				]) {
					if (!graph.some((entry) => entry.root === root))
						throw new Error(`Missing terminal contract consumer ${root}`);
					selected.add(root);
					reasons.push(`${root} <- ${changed}`);
				}
				continue;
			}
			const contractConsumers = sharedContractInputs.find(([input]) => input === changed)?.[1];
			if (contractConsumers) {
				for (const root of contractConsumers) {
					if (!graph.some((entry) => entry.root === root))
						throw new Error(`Missing shared contract consumer ${root}`);
					selected.add(root);
					reasons.push(`${root} <- ${changed}`);
				}
				continue;
			}
			const owner = owners.find((entry) => changed.startsWith(`${entry.root}/`));
			if (!owner) throw new Error(`Unknown or shared input: ${changed}`);
			selected.add(owner.root);
			reasons.push(`${owner.root} <- ${changed}`);
			for (const [input, consumer] of foreignPaths) {
				if (!matchesInput(changed, input)) continue;
				if (!graph.some((entry) => entry.root === consumer))
					throw new Error(`Missing foreign input consumer ${consumer}`);
				selected.add(consumer);
				reasons.push(`${consumer} <- ${changed}`);
			}
		}
		let size;
		do {
			size = selected.size;
			for (const entry of graph) {
				const dependency = entry.dependencies.find((root) => selected.has(root));
				if (dependency !== undefined && !selected.has(entry.root)) {
					selected.add(entry.root);
					reasons.push(`${entry.root} <- ${dependency}`);
				}
			}
		} while (selected.size !== size);
		for (const [witness, provider] of testWitnesses) {
			if (!selected.has(provider) || selected.has(witness)) continue;
			if (!graph.some((entry) => entry.root === witness))
				throw new Error(`Missing test witness ${witness}`);
			selected.add(witness);
			reasons.push(`${witness} <- ${provider} (test witness)`);
		}
		return Object.freeze({
			all: false,
			pnpm: Object.freeze(
				graph
					.filter((entry) => entry.kind === 'pnpm' && selected.has(entry.root))
					.map((entry) => entry.name)
					.toSorted(),
			),
			cargo: Object.freeze(
				graph
					.filter((entry) => entry.kind === 'cargo' && selected.has(entry.root))
					.map((entry) => entry.name)
					.toSorted(),
			),
			// A vendored owner has no suite to leave unexecuted.
			unaffected: Object.freeze(
				graph
					.filter((entry) => entry.kind !== 'vendored' && !selected.has(entry.root))
					.map((entry) => entry.root)
					.toSorted(),
			),
			tooling: Object.freeze(toolingSuiteNames.filter((name) => tooling.has(name))),
			completeRustLint,
			posixCompile,
			tla,
			// The export checker reads each library package's manifest and build output.
			packageExports:
				tooling.size === toolingSuites.length ||
				[...selected].some((root) => root.startsWith('packages/')),
			reasons: Object.freeze(reasons),
		});
	} catch (error) {
		return Object.freeze({ all: true, tla, reasons: Object.freeze([error.message]) });
	}
}

export function resolveVerificationScope(root, reference) {
	try {
		const { base, paths } = changedVerificationPaths(root, reference);
		return Object.freeze({
			...selectVerificationScope(readVerificationGraph(root), paths),
			base,
			changedPathCount: paths.length,
		});
	} catch (error) {
		if (error instanceof UnboundedBaseError) throw error;
		return Object.freeze({ all: true, reasons: Object.freeze([error.message]) });
	}
}

export function verificationOptions(args) {
	const options = { verbose: false, plan: false, changedSince: undefined };
	const values = args.filter((value) => value !== '--');
	for (let index = 0; index < values.length; index += 1) {
		if (values[index] === '--verbose') options.verbose = true;
		else if (values[index] === '--plan') options.plan = true;
		else if (
			values[index] === '--changed-since' &&
			options.changedSince === undefined &&
			values[index + 1] &&
			!values[index + 1].startsWith('-')
		)
			options.changedSince = values[++index];
		else throw new Error(`Unknown or incomplete verification option: ${values[index]}`);
	}
	if (options.plan && options.changedSince === undefined)
		throw new Error('--plan requires --changed-since');
	return options;
}
