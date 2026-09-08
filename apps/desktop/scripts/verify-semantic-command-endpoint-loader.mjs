import { execFile } from 'node:child_process';
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';

import { windowsNodeApiTarget } from './build-semantic-command-endpoint.mjs';

const executeFile = promisify(execFile);
const [addonPath, electronExecutable] = process.argv.slice(2);
if (
	addonPath === undefined ||
	electronExecutable === undefined ||
	!path.isAbsolute(addonPath) ||
	!path.isAbsolute(electronExecutable)
) {
	throw new Error('Absolute endpoint addon and Electron executable paths are required.');
}

const probe = `
const assert = require('node:assert/strict');
const addon = require(process.argv[1]);
assert.deepEqual(Object.getOwnPropertyNames(addon).sort(), [
  'acceptBefore', 'closeServer', 'createServer', 'disconnectServer', 'exchangeAuthenticated',
  'prepareRuntimeDirectory', 'publishLocator', 'readBefore', 'readLocator', 'removeLocator', 'writeBefore',
]);
for (const name of Object.getOwnPropertyNames(addon)) assert.equal(typeof addon[name], 'function');
console.log(process.versions.electron === undefined ? 'Node addon load verified.' : 'Electron addon load verified.');
`;
for (const executable of [process.execPath, electronExecutable]) {
	const { stdout } = await executeFile(executable, ['-e', probe, addonPath], {
		encoding: 'utf8',
		env: { ...process.env, ELECTRON_RUN_AS_NODE: '1' },
	});
	process.stdout.write(stdout);
}

if (process.platform === 'win32') {
	const fixture = fileURLToPath(
		new URL(
			'../src/native/semantic-command-endpoint/__tests__/node_api_missing_host.c',
			import.meta.url,
		),
	);
	const testRoot = await mkdtemp(path.join(tmpdir(), 'ob-node-api-'));
	try {
		for (const partial of [false, true]) {
			const executable = path.join(testRoot, partial ? 'partial.exe' : 'empty.exe');
			await executeFile('zig', [
				'cc',
				'-std=c17',
				'-O2',
				'-municode',
				'-Wall',
				'-Wextra',
				'-Wpedantic',
				'-Werror',
				'-target',
				windowsNodeApiTarget(process.arch),
				...(partial ? ['-DOB_PARTIAL_NODE_API'] : []),
				fixture,
				'-o',
				executable,
			]);
			await executeFile(executable, [addonPath]);
		}
		console.info('Windows missing Node-API host exports refused before registration.');
	} finally {
		await rm(testRoot, { recursive: true, force: true });
	}
}
