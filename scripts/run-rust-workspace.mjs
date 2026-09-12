import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { checkVerbose, runChecks } from './check-output.mjs';

const repositoryRoot = resolve(import.meta.dirname, '..');
const task = process.argv[2];
const verbose = checkVerbose(process.argv.slice(3));
const commands = new Map([
	['build', ['build', '--workspace', '--all-targets', '--all-features', '--locked']],
	['format:check', ['fmt', '--all', '--', '--check']],
	['format', ['fmt', '--all']],
	[
		'lint',
		[
			'clippy',
			'--workspace',
			'--all-targets',
			'--all-features',
			'--locked',
			'--',
			'-D',
			'warnings',
		],
	],
	['test', ['test', '--workspace', '--all-targets', '--all-features', '--locked']],
]);
const command = commands.get(task);

if (command === undefined) {
	throw new Error(`Unknown Rust workspace task: ${task ?? '<missing>'}`);
}

const toolchain = readFileSync(resolve(repositoryRoot, 'rust-toolchain.toml'), 'utf8');
const pinnedChannel = toolchain.match(/^channel = "([^"]+)"$/mu)?.[1];
if (pinnedChannel === undefined) {
	throw new Error('rust-toolchain.toml must declare one exact channel');
}

const rustcVersion = execFileSync('rustc', ['--version'], {
	cwd: repositoryRoot,
	encoding: 'utf8',
}).trim();
if (!rustcVersion.startsWith(`rustc ${pinnedChannel} `)) {
	throw new Error(`Expected rustc ${pinnedChannel}, received ${rustcVersion}`);
}

const metadata = JSON.parse(
	execFileSync('cargo', ['metadata', '--format-version', '1', '--no-deps', '--locked', '--quiet'], {
		cwd: repositoryRoot,
		encoding: 'utf8',
	}),
);

if (!Array.isArray(metadata.packages)) {
	throw new TypeError('Cargo metadata did not contain a packages array');
}

if (metadata.packages.length === 0) {
	process.stdout.write(`Verified empty Rust workspace for ${task}\n`);
} else {
	process.exitCode = await runChecks({
		name: `rust:${task}`,
		steps: [
			{ name: `cargo ${task}`, command: 'cargo', args: command },
			...(task === 'test'
				? [
						{
							name: 'cargo documentation tests',
							command: 'cargo',
							args: ['test', '--workspace', '--doc', '--all-features', '--locked'],
						},
					]
				: []),
		],
		cwd: repositoryRoot,
		verbose,
	});
}
