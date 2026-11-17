import { spawnSync } from 'node:child_process';
import { copyFileSync, lstatSync, mkdirSync, mkdtempSync, realpathSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { stageConptyBundle, verifyConptyBundle } from './windows-conpty.mjs';

const temporaryPrefix = 'orchbee-keeper-test-';
const fixtures = path.resolve(
	path.dirname(fileURLToPath(import.meta.url)),
	'../../../schemas/keeper/v1/fixtures',
);

function removeOwnedTemporary(directory, parent) {
	const actual = realpathSync(directory);
	if (path.dirname(actual) !== parent || !path.basename(actual).startsWith(temporaryPrefix)) {
		throw new Error('Refusing cleanup outside owned Keeper test directory');
	}
	// A staged OpenConsole.exe or a Keeper can outlive the driver for a moment, and Windows refuses
	// to delete a running image (EPERM), so the removal retries until they have exited.
	rmSync(actual, { recursive: true, maxRetries: 20, retryDelay: 250 });
}

function stageFile(source, directory, name) {
	const resolved = realpathSync(source);
	if (!lstatSync(resolved).isFile()) throw new Error(`${name} is not a file`);
	const staged = path.join(directory, name);
	copyFileSync(resolved, staged);
	return staged;
}

if (
	process.platform !== 'win32' ||
	process.arch !== 'x64' ||
	process.argv.length !== 6 ||
	process.argv[2] !== '--test-executable' ||
	process.argv[4] !== '--keeper-executable'
) {
	throw new Error(
		'Usage: run-keeper-test.mjs --test-executable TEST_EXECUTABLE --keeper-executable HOST_EXECUTABLE',
	);
}

const temporaryParent = realpathSync(os.tmpdir());
const temporary = mkdtempSync(path.join(temporaryParent, temporaryPrefix));
try {
	// The stand-in Hosts are the test executable itself, so the pinned conpty sits beside it.
	const executable = stageFile(process.argv[3], temporary, 'terminal-host-keeper-test.exe');
	const keeper = stageFile(process.argv[5], temporary, 'orchbee-terminal-host.exe');
	const bundle = path.join(temporary, 'conpty');
	await stageConptyBundle(bundle);
	verifyConptyBundle(bundle);
	const work = path.join(temporary, 'work');
	mkdirSync(work);

	const result = spawnSync(
		executable,
		['--keeper', keeper, '--fixtures', fixtures, '--work', work],
		{ stdio: 'inherit', timeout: 600_000, windowsHide: true },
	);
	if (result.error !== undefined) throw result.error;
	if (result.status !== 0) {
		throw new Error(`Keeper test exited with status ${result.status ?? 'unknown'}`);
	}
} finally {
	removeOwnedTemporary(temporary, temporaryParent);
}
