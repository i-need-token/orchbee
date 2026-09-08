import { execFile } from 'node:child_process';
import { access } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';

import { buildSemanticCommandEndpoint } from './build-semantic-command-endpoint.mjs';

const executeFile = promisify(execFile);
const desktopRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = path.resolve(desktopRoot, '..', '..');

async function executePnpm(arguments_) {
	const options = { cwd: repositoryRoot, encoding: 'utf8' };
	if (process.platform === 'win32') {
		return executeFile(
			process.env.ComSpec ?? 'cmd.exe',
			['/d', '/s', '/c', 'pnpm.cmd', ...arguments_],
			options,
		);
	}
	return executeFile('pnpm', arguments_, options);
}

const addonPath = await buildSemanticCommandEndpoint();
const runtimeVerifier = path.join(
	desktopRoot,
	'scripts',
	'verify-semantic-command-endpoint-runtime.mjs',
);
const localHttpVerifier = path.join(
	desktopRoot,
	'scripts',
	'verify-semantic-command-local-http-runtime.mjs',
);
const cliRuntimeVerifier = path.join(
	desktopRoot,
	'scripts',
	'verify-semantic-command-cli-runtime.mjs',
);
const cliEntryPath = path.join(repositoryRoot, 'apps', 'cli', 'dist', 'main', 'orchbee-cli.cjs');
const cliLauncherPath = path.join(
	repositoryRoot,
	'apps',
	'cli',
	'resources',
	process.platform === 'win32' ? 'orchbee-cli.cmd' : 'orchbee-cli',
);
const workerPath = path.join(
	desktopRoot,
	'dist',
	'main',
	'semantic-command-local-endpoint-worker.cjs',
);
try {
	await access(workerPath);
} catch {
	await executePnpm(['--filter', '@orchbee/desktop...', 'build']);
}
try {
	await access(cliEntryPath);
} catch {
	await executePnpm(['--filter', '@orchbee/cli...', 'build']);
}
const electronExecutable = createRequire(import.meta.url)('electron');
if (typeof electronExecutable !== 'string') {
	throw new Error('The pinned Electron executable is unavailable.');
}
const { stdout: loaderOutput } = await executeFile(
	process.execPath,
	[
		path.join(desktopRoot, 'scripts', 'verify-semantic-command-endpoint-loader.mjs'),
		addonPath,
		electronExecutable,
	],
	{ encoding: 'utf8' },
);
process.stdout.write(loaderOutput);
if (process.platform === 'win32') {
	const { stdout: securityOutput } = await executeFile(
		process.execPath,
		[path.join(desktopRoot, 'scripts', 'verify-semantic-command-endpoint-windows-security.mjs')],
		{ encoding: 'utf8' },
	);
	process.stdout.write(securityOutput);
}
for (const [command, arguments_, environment] of [
	[process.execPath, [runtimeVerifier, addonPath], process.env],
	[process.execPath, [localHttpVerifier, addonPath, workerPath], process.env],
	[electronExecutable, [runtimeVerifier, addonPath], { ...process.env, ELECTRON_RUN_AS_NODE: '1' }],
	[
		electronExecutable,
		[localHttpVerifier, addonPath, workerPath],
		{ ...process.env, ELECTRON_RUN_AS_NODE: '1' },
	],
]) {
	const { stderr, stdout } = await executeFile(command, arguments_, {
		encoding: 'utf8',
		env: environment,
	});
	if (stdout.length > 0) process.stdout.write(stdout);
	if (stderr.length > 0) process.stderr.write(stderr);
}
const { stderr, stdout } = await executeFile(
	process.execPath,
	[cliRuntimeVerifier, addonPath, workerPath, cliEntryPath, cliLauncherPath, electronExecutable],
	{ encoding: 'utf8', env: process.env },
);
if (stdout.length > 0) process.stdout.write(stdout);
if (stderr.length > 0) process.stderr.write(stderr);
