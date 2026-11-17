import { spawnSync } from 'node:child_process';
import { constants, copyFileSync, lstatSync, mkdtempSync, realpathSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { stageConptyBundle, verifyConptyBundle } from './windows-conpty.mjs';

const temporaryPrefix = 'orchbee-keeper-link-test-';
const witnessTimeoutMilliseconds = 300_000;

function removeOwnedTemporary(directory, parent) {
	const actual = realpathSync(directory);
	if (path.dirname(actual) !== parent || !path.basename(actual).startsWith(temporaryPrefix)) {
		throw new Error('Refusing cleanup outside owned Keeper link test directory');
	}
	// A staged OpenConsole.exe or a Keeper can outlive the test for a moment, and Windows refuses
	// to delete a running image (EPERM), so the removal retries until they have exited.
	rmSync(actual, { recursive: true, maxRetries: 20, retryDelay: 250 });
}

if (
	process.platform !== 'win32' ||
	process.arch !== 'x64' ||
	process.argv.length !== 4 ||
	process.argv[2] !== '--test-executable'
) {
	throw new Error('Usage: run-keeper-link-test.mjs --test-executable TEST_EXECUTABLE');
}

const source = realpathSync(process.argv[3]);
if (!lstatSync(source).isFile()) throw new Error('Keeper link test executable is not a file');
const temporaryParent = realpathSync(os.tmpdir());
const temporary = mkdtempSync(path.join(temporaryParent, temporaryPrefix));
let failure;
try {
	// The Host's service runs in the test, so its terminals use the pinned conpty beside it.
	const executable = path.join(temporary, 'terminal-host-keeper-link-tests.exe');
	copyFileSync(source, executable, constants.COPYFILE_EXCL);
	const bundle = path.join(temporary, 'conpty');
	await stageConptyBundle(bundle);
	verifyConptyBundle(bundle);

	const result = spawnSync(executable, [], {
		stdio: 'inherit',
		timeout: witnessTimeoutMilliseconds,
		windowsHide: true,
	});
	if (result.error !== undefined) throw result.error;
	if (result.status !== 0) {
		throw new Error(`Keeper link test exited with status ${result.status ?? 'unknown'}`);
	}
} catch (error) {
	failure = error;
}

try {
	removeOwnedTemporary(temporary, temporaryParent);
} catch (cleanupError) {
	if (failure !== undefined) {
		throw new AggregateError(
			[failure, cleanupError],
			'Keeper link test failed and its owned temporary directory could not be removed',
		);
	}
	throw cleanupError;
}
if (failure !== undefined) throw failure;
