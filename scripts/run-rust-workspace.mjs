import { execFileSync } from 'node:child_process';
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

import { checkVerbose, runChecks } from './check-output.mjs';

const repositoryRoot = resolve(import.meta.dirname, '..');
const task = process.argv[2];
const selectedPackages = [];
const presentationArguments = [];
const arguments_ = process.argv.slice(3).filter((argument) => argument !== '--');
for (let index = 0; index < arguments_.length; index += 1) {
	if (arguments_[index] === '--package') {
		const name = arguments_[++index];
		if (task !== 'test' || name === undefined || !/^[a-z\d][a-z\d_-]*$/u.test(name))
			throw new Error('--package requires a Cargo package name and the test task');
		selectedPackages.push(name);
	} else presentationArguments.push(arguments_[index]);
}
const verbose = checkVerbose(presentationArguments);
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

for (const name of selectedPackages) {
	if (
		metadata.packages.filter(
			(entry) => entry.name === name && metadata.workspace_members.includes(entry.id),
		).length !== 1
	)
		throw new Error(`Unknown or ambiguous Cargo workspace package: ${name}`);
}
const workspaceArguments = selectedPackages.length
	? [...new Set(selectedPackages)].flatMap((name) => ['--package', name])
	: ['--workspace'];
const scopedCommand = command.flatMap((argument) =>
	argument === '--workspace' ? workspaceArguments : [argument],
);
const documentationArguments = selectedPackages.length
	? metadata.packages
			.filter(
				(entry) =>
					selectedPackages.includes(entry.name) &&
					entry.targets.some(
						(target) =>
							target.doctest &&
							target.kind.some((kind) =>
								['lib', 'rlib', 'dylib', 'staticlib', 'cdylib', 'proc-macro'].includes(kind),
							),
					),
			)
			.flatMap((entry) => ['--package', entry.name])
	: ['--workspace'];

if (metadata.packages.length === 0) {
	process.stdout.write(`Verified empty Rust workspace for ${task}\n`);
} else {
	process.exitCode = await runChecks({
		name: `rust:${task}`,
		steps: [
			{ name: `cargo ${task}`, command: 'cargo', args: scopedCommand },
			...(task === 'test' && documentationArguments.length
				? [
						{
							name: 'cargo documentation tests',
							command: 'cargo',
							args: ['test', ...documentationArguments, '--doc', '--all-features', '--locked'],
						},
					]
				: []),
		],
		cwd: repositoryRoot,
		verbose,
	});
}
