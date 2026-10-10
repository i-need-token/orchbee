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

export function checkSteps(task, scope) {
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
		[
			'verify',
			[
				'format:check',
				'test:tla:inventory',
				'build',
				'lint',
				'typecheck',
				'test',
				'test:exports',
			].map((name) => pnpmStep(name)),
		],
		[
			'test',
			[
				// Small independent gates overlap freely.
				{
					concurrent: [
						'test:check-output',
						'test:rust-runner',
						'test:lint-rules',
						'test:documentation-pairing',
						'test:ghostty-source',
						'test:style-contract',
						'test:storage-contract',
						'test:experience-contract',
						'test:stage-native-site',
					].map((name) => pnpmStep(name)),
				},
				// Native process witnesses spend real deadlines on startup and cohort verification.
				// Isolated build caches do not isolate them from CPU-heavy Node suites.
				pnpmStep('workspace tests', ['-r', 'test']),
				nodeStep('Rust workspace tests', ['scripts/run-rust-workspace.mjs', 'test']),
				pnpmStep('native cross-compilation checks', ['-r', '--if-present', 'test:cross']),
			],
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
			'test:documentation-pairing',
			[
				nodeStep('documentation-pairing tests', [
					'--test',
					'scripts/documentation-pairing.test.mjs',
				]),
			],
		],
		[
			'test:ghostty-source',
			[nodeStep('Ghostty source tests', ['--test', 'scripts/ghostty-source.test.mjs'])],
		],
		[
			'test:stage-native-site',
			[nodeStep('native-site staging tests', ['--test', 'scripts/stage-native-site.test.mjs'])],
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
			return steps.flatMap((step) => (step.name === 'test' ? checkSteps('test', scope) : [step]));
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
							nodeStep('affected Rust workspace tests', [
								'scripts/run-rust-workspace.mjs',
								'test',
								...scope.cargo.flatMap((name) => ['--package', name]),
							]),
						]
					: []),
				...(scope.pnpm.length
					? [
							pnpmStep('affected native cross-compilation checks', [
								'-r',
								...scope.pnpm.flatMap((name) => ['--filter', name]),
								'--if-present',
								'test:cross',
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
		const options =
			name === 'verify'
				? verificationOptions(process.argv.slice(3))
				: { verbose: checkVerbose(process.argv.slice(3)) };
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
					steps: checkSteps(name, scope),
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
