import { resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

import { checkVerbose, runChecks } from '../../../scripts/check-output.mjs';

export function hostChecks(task, platform = process.platform, arch = process.arch) {
	if (task !== 'build' && task !== 'test') throw new Error(`Unknown Runtime check: ${task}`);
	const supported =
		(platform === 'darwin' && arch === 'arm64') || (platform === 'linux' && arch === 'x64');
	const steps = [];
	if (supported) {
		steps.push({
			name: 'Runtime native build',
			command: 'zig',
			args: task === 'build' ? ['build', '-Doptimize=ReleaseSafe'] : ['build'],
		});
		if (task === 'test') {
			steps.push(
				{ name: 'Runtime native tests', command: 'zig', args: ['build', 'test'] },
				{
					name: 'Runtime process conformance',
					command: process.execPath,
					args: ['scripts/run-runtime-conformance.mjs'],
				},
			);
		}
	}
	if (task === 'test') {
		steps.push({
			name: 'Runtime release and host-check tests',
			command: process.execPath,
			args: ['--test', 'scripts/ssh-runtime-release.test.mjs', 'scripts/run-host-checks.test.mjs'],
		});
	}
	return {
		steps,
		notice: supported
			? undefined
			: `Skipping SSH Runtime native ${task} on ${platform}-${arch}: supported remote hosts are darwin-arm64 and linux-x64. Release tests and test:cross remain required; this is not native Runtime validation.`,
	};
}

if (process.argv[1] !== undefined && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
	try {
		const { steps, notice } = hostChecks(process.argv[2]);
		if (notice !== undefined) process.stdout.write(`${notice}\n`);
		process.exitCode = await runChecks({
			name: 'SSH Runtime',
			steps,
			cwd: resolve(import.meta.dirname, '..'),
			verbose: checkVerbose(process.argv.slice(3)),
		});
	} catch (error) {
		process.stderr.write(`${error.message}\n`);
		process.exitCode = 1;
	}
}
