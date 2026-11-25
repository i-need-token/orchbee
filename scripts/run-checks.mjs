import { readFileSync } from 'node:fs';
import { basename, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { checkVerbose, runChecks } from './check-output.mjs';
import { testParts as rustTestParts } from './rust-test-parts.mjs';
import {
	resolveVerificationScope,
	toolingSuiteNames,
	verificationOptions,
} from './verification-scope.mjs';

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

// Markdown links and the Chinese allowlist cover every tracked file, so this suite is a repository
// check and runs for every change; the other root suites test the gate's own tools.
const repositorySuite = 'test:lint-rules';
const rustRunner = 'scripts/run-rust-workspace.mjs';

// One step per command of a root script that chains commands with `&&`. The changed gate narrows
// or leaves out single commands of `format:check` and `lint`; reading the script keeps it from
// drifting away from the complete one.
function scriptSteps(name) {
	const scripts = JSON.parse(
		readFileSync(new URL('../package.json', import.meta.url), 'utf8'),
	).scripts;
	return scripts[name].split(' && ').map((part) => {
		const [program, ...args] = part.split(' ');
		if (program === 'pnpm') return pnpmStep(args.length === 1 ? args[0] : part, args);
		if (program === 'node') return nodeStep(part, args);
		return pnpmStep(part, ['exec', program, ...args]);
	});
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

// A part name is checked here too, so a typo fails before the long preparation steps run for it.
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
		'build:packages',
		'lint',
		'typecheck',
		'test',
		'test:exports',
	];
	// Every `test` step that needs no Cargo: root tooling tests, then the pnpm workspace suites.
	const nodeSuites = [
		// Small independent gates overlap freely.
		{ concurrent: [repositorySuite, ...toolingSuiteNames].map((name) => pnpmStep(name)) },
		pnpmStep('workspace tests', ['-r', 'test']),
	];
	const tasks = new Map([
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
	]);
	const steps = tasks.get(task);
	if (steps === undefined) {
		throw new Error(`Unknown check task: ${task ?? '<missing>'}`);
	}
	if (scope !== undefined && task === 'verify') {
		const verifySteps = scope.all ? steps : changedVerifySteps(scope, platform);
		// No other gate runs TLA+: only a change to a model or its tooling runs the changed models.
		return scope.tla
			? [
					...verifySteps,
					nodeStep('changed TLA+ models', [
						'scripts/tla/run-models.mjs',
						'--changed-since',
						scope.base ?? 'HEAD',
					]),
				]
			: verifySteps;
	}
	if (scope !== undefined && !scope.all && task === 'test')
		return changedTestSteps(scope, platform);
	return steps;
}

// The changed gate. Format, build, lint and type checks of TypeScript, styles and dependencies
// always run complete, because their inputs cross every workspace; the scope narrows the Rust
// checks, the root tooling suites, the test suites and the export check.
function changedVerifySteps(scope, platform) {
	const packages = scope.cargo.flatMap((name) => ['--package', name]);
	let rustLint = [];
	if (scope.completeRustLint) rustLint = [pnpmStep('lint:rust')];
	else if (scope.cargo.length > 0)
		rustLint = [nodeStep('affected Rust lint', [rustRunner, 'lint', ...packages])];
	return [
		...scriptSteps('format:check').filter(
			(step) => scope.cargo.length > 0 || !step.args.includes(rustRunner),
		),
		pnpmStep('build:packages'),
		...scriptSteps('lint').flatMap((step) => (step.name === 'lint:rust' ? rustLint : [step])),
		pnpmStep('typecheck'),
		...changedTestSteps(scope, platform),
		...(scope.packageExports ? [pnpmStep('test:exports')] : []),
	];
}

function changedTestSteps(scope, platform) {
	return [
		{
			concurrent: [repositorySuite, ...scope.tooling].map((name) => pnpmStep(name)),
		},
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
						rustRunner,
						'test',
						...scope.cargo.flatMap((name) => ['--package', name]),
					]),
				]
			: []),
		// The host suite compiles the Terminal Host for its own system only. Compiling its tests for
		// the two POSIX release systems catches a break there before CI does.
		...(scope.posixCompile
			? ['x86_64-linux-gnu', 'aarch64-macos'].map((target) =>
					pnpmStep(`Terminal Host test compilation for ${target}`, [
						'--filter',
						'@orchbee/terminal-host',
						'exec',
						'zig',
						'build',
						'test-compile',
						'-j2',
						`-Dtarget=${target}`,
					]),
				)
			: []),
	];
}

if (process.argv[1] !== undefined && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
	try {
		const name = process.argv[2];
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
	} catch (error) {
		process.stderr.write(`${error.message}\n`);
		process.exitCode = 1;
	}
}
