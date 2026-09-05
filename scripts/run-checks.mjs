import { basename, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { checkVerbose, runChecks } from './check-output.mjs';

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

export function checkSteps(task) {
	const tasks = new Map([
		[
			'verify',
			[
				'format:check',
				'lint',
				'test:tla:inventory',
				'build',
				'typecheck',
				'test',
				'test:exports',
			].map((name) => pnpmStep(name)),
		],
		[
			'test',
			[
				...[
					'test:check-output',
					'test:lint-rules',
					'test:documentation-pairing',
					'test:ghostty-source',
					'test:style-contract',
					'test:storage-contract',
					'test:experience-contract',
					'test:ssh-environment-release',
				].map((name) => pnpmStep(name)),
				pnpmStep('workspace tests', ['-r', 'test']),
				nodeStep('Rust workspace tests', ['scripts/run-rust-workspace.mjs', 'test']),
				pnpmStep('SSH Project enrollment system witness', [
					'--filter',
					'@orchbee/desktop',
					'test:system:ssh-project-enrollment',
				]),
				pnpmStep('native cross-compilation checks', ['-r', '--if-present', 'test:cross']),
			],
		],
		[
			'test:check-output',
			[nodeStep('check-output tests', ['--test', 'scripts/check-output.test.mjs'])],
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
			'test:ssh-environment-release',
			[
				pnpmStep('SSH environment protocol build', [
					'--filter',
					'@orchbee/ssh-environment-protocol',
					'build',
				]),
				nodeStep('SSH environment release tests', [
					'--test',
					'scripts/ssh-environment-release.test.mjs',
				]),
			],
		],
		[
			'test:tla:inventory',
			[
				nodeStep('TLA inventory tests', ['--test', 'scripts/tla/model-inventory.test.mjs']),
				nodeStep('TLA inventory check', ['scripts/tla/check-inventory.mjs']),
			],
		],
	]);
	const steps = tasks.get(task);
	if (steps === undefined) {
		throw new Error(`Unknown check task: ${task ?? '<missing>'}`);
	}
	return steps;
}

if (process.argv[1] !== undefined && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
	try {
		const name = process.argv[2];
		process.exitCode = runChecks({
			name,
			steps: checkSteps(name),
			verbose: checkVerbose(process.argv.slice(3)),
			cwd: resolve(import.meta.dirname, '..'),
		});
	} catch (error) {
		process.stderr.write(`${error.message}\n`);
		process.exitCode = 1;
	}
}
