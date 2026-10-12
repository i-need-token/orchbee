import { execFileSync } from 'node:child_process';
import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { isDeepStrictEqual } from 'node:util';

import { parsePairingManifest } from './documentation-pairing.mjs';

const packageName = /^(?:@[a-z\d._-]+\/)?[a-z\d][a-z\d._-]*$/u;
const manifestName =
	/(?:^|\/)(?:package\.json|Cargo\.toml|Cargo\.lock|pnpm-lock\.yaml|pnpm-workspace\.yaml)$/u;

// Runtime/process inputs not represented by package managers. The root verification owner
// maintains these edges with their consumers; a new foreign input needs a positive control.
const foreignDependencies = [
	['apps/orchbee', 'apps/orchbee-server'],
	['apps/orchbee', 'apps/orchbee-worker'],
	['apps/orchbee', 'apps/terminal-host'],
	['apps/orchbee-server', 'apps/orchbee-worker'],
	['apps/orchbee-server', 'apps/terminal-host'],
	['apps/orchbee-worker', 'apps/terminal-host'],
	['apps/desktop', 'apps/terminal-host'],
	['apps/desktop', 'apps/web'],
	['crates/terminal-protocol', 'packages/terminal-protocol'],
];
const foreignPaths = [
	['apps/desktop/resources/', 'apps/orchbee'],
	[
		'packages/tab-strip/src/tab-strip/model/__tests__/fixtures/tab-close-targets.json',
		'apps/orchbee',
	],
];

// Exact shared-input ownership; new standards/helpers require review rather than
// inheriting a directory-wide exemption from product verification.
const tuiStandards = new Set(
	[
		'keyboard-input',
		'layout',
		'mouse-input',
		'overlays-and-status',
		'terminal-environment',
		'verification',
		'visual-foundations',
	].flatMap((name) => [`docs/tui/${name}.md`, `docs/zh-CN/tui/${name}.md`]),
);
const rootToolInputs = new Set([
	'scripts/run-checks.mjs',
	'scripts/run-rust-workspace.mjs',
	'scripts/run-rust-workspace.test.mjs',
	'scripts/check-output.mjs',
	'scripts/check-output.test.mjs',
	'scripts/verification-scope.mjs',
	'scripts/verification-scope.test.mjs',
	'scripts/documentation-pairing.mjs',
	'scripts/documentation-pairing.test.mjs',
	'docs/VERIFICATION.md',
	'docs/zh-CN/VERIFICATION.md',
	'docs/MODELING-AND-CORRECTNESS.md',
	'docs/zh-CN/MODELING-AND-CORRECTNESS.md',
	...[
		'check-inventory.mjs',
		'document-scopes.mjs',
		'document-scopes.test.mjs',
		'execution-properties.test.mjs',
		'model-cache-cli.test.mjs',
		'model-mutations.test.mjs',
		'test-support/child-process.mjs',
		'toolchain.mjs',
		'toolchain.test.mjs',
		'model-cache-identity.mjs',
		'model-cache.mjs',
		'model-cache.test.mjs',
		'model-checks.mjs',
		'model-checks.test.mjs',
		'model-inputs.mjs',
		'model-inventory.mjs',
		'model-inventory.test.mjs',
		'model-records.mjs',
		'model-records.test.mjs',
		'models.json',
		'README.md',
		'README.zh-CN.md',
		'run-models.mjs',
		'run-models.test.mjs',
		'select-changed-models.mjs',
	].map((name) => `scripts/tla/${name}`),
]);
const pairingPath = 'scripts/documentation-pairing.json';

function pairingChanges(versions) {
	if (!versions) throw new Error('Documentation pairing history is unavailable');
	const manifests = [versions.before, ...(versions.intermediate ?? []), versions.after].map(
		parsePairingManifest,
	);
	return [...new Set(manifests.flatMap((manifest) => Object.keys(manifest.pairs)))].filter(
		(source) =>
			manifests.some(
				(manifest) => !isDeepStrictEqual(manifests[0].pairs[source], manifest.pairs[source]),
			),
	);
}

function capture(root, command, args) {
	return execFileSync(command, args, { cwd: root, encoding: 'utf8', maxBuffer: 16 * 1024 ** 2 });
}

export function changedVerificationPaths(root, reference) {
	const base = capture(root, 'git', [
		'rev-parse',
		'--verify',
		'--end-of-options',
		`${reference}^{commit}`,
	]).trim();
	capture(root, 'git', ['merge-base', '--is-ancestor', base, 'HEAD']);
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
	const metadata = JSON.parse(
		capture(root, 'cargo', [
			'metadata',
			'--format-version',
			'1',
			'--no-deps',
			'--locked',
			'--quiet',
		]),
	);
	const members = metadata.packages.filter((entry) =>
		metadata.workspace_members.includes(entry.id),
	);
	const byPath = new Map(
		members.map((entry) => [path.resolve(path.dirname(entry.manifest_path)), entry]),
	);
	for (const entry of members) {
		const owner = path.relative(root, path.dirname(entry.manifest_path)).replaceAll('\\', '/');
		const dependencies = entry.dependencies
			.filter((dependency) => dependency.path !== undefined)
			.map((dependency) => {
				const target = byPath.get(path.resolve(dependency.path));
				if (!target)
					throw new Error(`Unmapped local Cargo dependency ${entry.name}: ${dependency.path}`);
				return path.relative(root, path.dirname(target.manifest_path)).replaceAll('\\', '/');
			});
		graph.push({ root: owner, name: entry.name, kind: 'cargo', dependencies });
	}
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

function validateGraph(graph) {
	const roots = new Set(graph.map((entry) => entry.root));
	const names = new Set(graph.map((entry) => `${entry.kind}:${entry.name}`));
	if (roots.size !== graph.length || names.size !== graph.length)
		throw new Error('Ambiguous workspace owner');
	for (const entry of graph) {
		if (
			!packageName.test(entry.name) ||
			!/^(?:apps|packages|crates)\/[a-z\d-]+$/u.test(entry.root) ||
			!['pnpm', 'cargo'].includes(entry.kind) ||
			!Array.isArray(entry.dependencies) ||
			entry.dependencies.some((dependency) => !roots.has(dependency))
		)
			throw new Error(`Unsupported workspace owner ${entry.root}`);
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

export function selectVerificationScope(graph, paths, pairingVersions) {
	try {
		validateGraph(graph);
		const selected = new Set();
		const reasons = [];
		const ownedPaths = paths.flatMap((changed) =>
			changed === pairingPath ? pairingChanges(pairingVersions) : [changed],
		);
		for (const changed of ownedPaths) {
			if (manifestName.test(changed)) throw new Error(`Dependency manifest changed: ${changed}`);
			if (rootToolInputs.has(changed)) {
				reasons.push(`root tooling <- ${changed}`);
				continue;
			}
			const owner = graph.find((entry) =>
				tuiStandards.has(changed)
					? entry.root === 'apps/orchbee'
					: changed.startsWith(`${entry.root}/`),
			);
			if (!owner) throw new Error(`Unknown or shared input: ${changed}`);
			selected.add(owner.root);
			reasons.push(`${owner.root} <- ${changed}`);
			for (const [input, consumer] of foreignPaths) {
				const matchesInput = input.endsWith('/') ? changed.startsWith(input) : changed === input;
				if (!matchesInput) continue;
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
			unaffected: Object.freeze(
				graph
					.filter((entry) => !selected.has(entry.root))
					.map((entry) => entry.root)
					.toSorted(),
			),
			reasons: Object.freeze(reasons),
		});
	} catch (error) {
		return Object.freeze({ all: true, reasons: Object.freeze([error.message]) });
	}
}

export function resolveVerificationScope(root, reference) {
	try {
		const { base, paths } = changedVerificationPaths(root, reference);
		const pairingVersions = paths.includes(pairingPath)
			? {
					before: capture(root, 'git', ['show', `${base}:${pairingPath}`]),
					intermediate: [
						capture(root, 'git', ['show', `HEAD:${pairingPath}`]),
						capture(root, 'git', ['show', `:${pairingPath}`]),
					],
					after: readFileSync(path.join(root, pairingPath), 'utf8'),
				}
			: undefined;
		return Object.freeze({
			...selectVerificationScope(readVerificationGraph(root), paths, pairingVersions),
			base,
			changedPathCount: paths.length,
		});
	} catch (error) {
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
