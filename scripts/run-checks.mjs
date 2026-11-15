import { basename, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { checkVerbose, runChecks } from './check-output.mjs';
import { resolveVerificationScope, verificationOptions } from './verification-scope.mjs';

function pnpmStep(name, args = [name]) {
	const cli = process.env.npm_execpath;
	// pnpm run supplies its JS entry, avoiding a .cmd shell boundary on Windows.
	return cli !== undefined && /^pnpm\.(?:c?js|mjs)$/u.test(basename(cli))
		? { name, command: process.execPath, args: [cli, ...args] }
		: { name, command: 'pnpm', args };
}

function nodeStep(name, args) {
	return { name, command: process.execPath, args };
}

function nativeTestPreparation(scope, platform) {
	// Worker witnesses launch the real Host; Linux and Windows CLI witnesses also start sibling
	// peers. Keep these inputs fresh without building unrelated apps or every Cargo target.
	const selected = (name) => scope === undefined || scope.all || scope.cargo.includes(name);
	const cliCohort = (platform === 'linux' || platform === 'win32') && selected('orchbee');
	return [
		...(selected('orchbee-worker') || cliCohort
			? [pnpmStep('prepare native test host', ['--filter', '@orchbee/terminal-host', 'build'])]
			: []),
		...(cliCohort
			? [
					{
						name: 'prepare native test peers',
						command: 'cargo',
						args: [
							'build',
							'--package',
							'orchbee-server',
							'--package',
							'orchbee-worker',
							'--bins',
							'--all-features',
							'--locked',
						],
					},
				]
			: []),
	];
}

// The Rust runner owns the test parts. This copy only rejects a typo before the long preparation
// steps would run for it.
const rustTestParts = ['workspace', 'client', 'client-commands'];

function rustNativeSteps(part, platform) {
	if (part !== undefined && !rustTestParts.includes(part))
		throw new Error(`--part expects one of: ${rustTestParts.join(', ')}`);
	return [
		// Lint covers the whole workspace, so of the parts only the workspace part runs it. Every
		// part keeps the whole preparation: a part must not differ from the unsplit task in what
		// its tests find built.
		...(part === undefined || part === 'workspace'
			? [nodeStep('Rust workspace lint', ['scripts/run-rust-workspace.mjs', 'lint'])]
			: []),
		...nativeTestPreparation(undefined, platform),
		nodeStep('Rust workspace tests', [
			'scripts/run-rust-workspace.mjs',
			'test',
			...(part === undefined ? [] : ['--part', part]),
		]),
	];
}

export function checkSteps(task, scope, platform = process.platform, rustPart) {
	if (task === 'verify:full') {
		return checkSteps('verify', undefined, platform).flatMap((step) => {
			if (step.name === 'build:packages') return [pnpmStep('build:full')];
			if (step.name === 'test') return [step, pnpmStep('test:cross')];
			return [step];
		});
	}
	const verifyStages = [
		'format:check',
		'test:tla:inventory',
		'build:packages',
		'lint',
		'typecheck',
		'test',
		'test:exports',
	];
	// Every `test` step that needs no Cargo: root tooling tests, then the pnpm workspace suites.
	const nodeSuites = [
		// Small independent gates overlap freely.
		{
			concurrent: [
				'test:check-output',
				'test:rust-runner',
				'test:rust-modules',
				'test:lint-rules',
				'test:dependency-scope',
				'test:ghostty-source',
				'test:style-contract',
				'test:storage-contract',
				'test:experience-contract',
				'test:stage-local-runtime',
				'test:release-feed',
				'test:agent-observation-adapters',
			].map((name) => pnpmStep(name)),
		},
		pnpmStep('workspace tests', ['-r', 'test']),
	];
	const tasks = new Map([
		[
			'test:tla',
			[
				pnpmStep('test:tla:inventory'),
				{
					concurrent: [
						nodeStep('TLA models', ['scripts/tla/run-models.mjs', '--all']),
						pnpmStep('test:tla:tooling'),
					],
				},
			],
		],
		['verify', verifyStages.map((name) => pnpmStep(name))],
		[
			// `verify` for a host whose Cargo suites are not release evidence: every stage except the
			// Cargo tests and their native prerequisites. Rust lint stays, so that host's Rust
			// configuration still compiles.
			'verify:no-rust-tests',
			verifyStages.flatMap((name) => (name === 'test' ? nodeSuites : [pnpmStep(name)])),
		],
		[
			'test',
			[
				// Native process witnesses spend real deadlines on startup and cohort verification.
				// Isolated build caches do not isolate them from CPU-heavy Node suites, so those finish first.
				...nodeSuites,
				...nativeTestPreparation(undefined, platform),
				nodeStep('Rust workspace tests', ['scripts/run-rust-workspace.mjs', 'test']),
			],
		],
		[
			// One OS-native Rust pass: platform-gated code gets Clippy and Cargo tests on its own host.
			// With `--part`, one of the runner's test parts; CI runs the parts as parallel jobs.
			'verify:rust-native',
			rustNativeSteps(rustPart, platform),
		],
		[
			'test:cross',
			[pnpmStep('native cross-compilation checks', ['-r', '--if-present', 'test:cross'])],
		],
		[
			'test:check-output',
			[
				nodeStep('check-output tests', [
					'--test',
					'scripts/check-output.test.mjs',
					'scripts/verification-scope.test.mjs',
				]),
			],
		],
		[
			'test:ghostty-source',
			[nodeStep('Ghostty source tests', ['--test', 'scripts/ghostty-source.test.mjs'])],
		],
		[
			'test:stage-local-runtime',
			[nodeStep('local-runtime staging tests', ['--test', 'scripts/stage-local-runtime.test.mjs'])],
		],
		[
			'test:tla:inventory',
			[
				nodeStep('TLA inventory and runner tests', [
					'--test',
					'--test-concurrency=4',
					'scripts/tla/model-inventory.test.mjs',
					'scripts/tla/model-checks.test.mjs',
					'scripts/tla/document-scopes.test.mjs',
					'scripts/tla/model-records.test.mjs',
					'scripts/tla/model-cache.test.mjs',
					'scripts/tla/execution-properties.test.mjs',
					'scripts/tla/run-models.test.mjs',
					'scripts/tla/toolchain.test.mjs',
				]),
				nodeStep('TLA inventory check', ['scripts/tla/check-inventory.mjs']),
			],
		],
	]);
	const steps = tasks.get(task);
	if (steps === undefined) {
		throw new Error(`Unknown check task: ${task ?? '<missing>'}`);
	}
	if (scope !== undefined && !scope.all) {
		if (task === 'verify')
			return steps.flatMap((step) =>
				step.name === 'test' ? checkSteps('test', scope, platform) : [step],
			);
		if (task === 'test')
			return [
				steps[0],
				...(scope.pnpm.length
					? [
							pnpmStep('affected workspace tests', [
								'-r',
								...scope.pnpm.flatMap((name) => ['--filter', name]),
								'test',
							]),
						]
					: []),
				...(scope.cargo.length
					? [
							...nativeTestPreparation(scope, platform),
							nodeStep('affected Rust workspace tests', [
								'scripts/run-rust-workspace.mjs',
								'test',
								...scope.cargo.flatMap((name) => ['--package', name]),
							]),
						]
					: []),
			];
	}
	return steps;
}

if (process.argv[1] !== undefined && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
	try {
		const name = process.argv[2];
		const started = performance.now();
		const root = resolve(import.meta.dirname, '..');
		const taskArguments = process.argv.slice(3);
		const partIndex = name === 'verify:rust-native' ? taskArguments.indexOf('--part') : -1;
		const rustPart = partIndex === -1 ? undefined : (taskArguments.splice(partIndex, 2)[1] ?? '');
		const options =
			name === 'verify'
				? verificationOptions(taskArguments)
				: { verbose: checkVerbose(taskArguments) };
		const scope =
			options.changedSince === undefined
				? undefined
				: resolveVerificationScope(root, options.changedSince);
		if (scope !== undefined)
			process.stdout.write(
				`VERIFY_PLAN ${JSON.stringify({ ...scope, reasons: scope.reasons.slice(0, 30), omittedReasons: Math.max(0, scope.reasons.length - 30), unaffected: scope.unaffected, mode: scope.all ? 'conservative-full' : 'affected' })}\n`,
			);
		process.exitCode = options.plan
			? 0
			: await runChecks({
					name,
					steps: checkSteps(name, scope, undefined, rustPart),
					verbose: options.verbose,
					cwd: root,
				});
		if (name === 'test:tla' && performance.now() - started > 60_000) {
			process.stderr.write(
				'TLA verification exceeded the 60 second budget; reduce model or tooling cost before accepting this gate.\n',
			);
			process.exitCode = 1;
		}
	} catch (error) {
		process.stderr.write(`${error.message}\n`);
		process.exitCode = 1;
	}
}
