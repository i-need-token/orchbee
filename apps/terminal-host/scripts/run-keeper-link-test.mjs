import { spawnSync } from 'node:child_process';
import { constants, copyFileSync, lstatSync, mkdtempSync, realpathSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { stageConptyBundle, verifyConptyBundle } from './windows-conpty.mjs';

const temporaryPrefix = 'orchbee-keeper-link-test-';
const witnessTimeoutMilliseconds = 600_000;
const usage =
	'Usage: run-keeper-link-test.mjs --test-executable TEST_EXECUTABLE [--host-executable HOST_EXECUTABLE]';

function removeOwnedTemporary(directory, parent) {
	const actual = realpathSync(directory);
	if (path.dirname(actual) !== parent || !path.basename(actual).startsWith(temporaryPrefix)) {
		throw new Error('Refusing cleanup outside owned Keeper link test directory');
	}
	// A staged OpenConsole.exe or a Keeper can outlive the test for a moment, and Windows refuses
	// to delete a running image (EPERM), so the removal retries until they have exited.
	rmSync(actual, { recursive: true, maxRetries: 20, retryDelay: 250 });
}

const options = process.argv.slice(2);
if (
	process.platform !== 'win32' ||
	process.arch !== 'x64' ||
	(options.length !== 2 && options.length !== 4) ||
	options[0] !== '--test-executable' ||
	(options.length === 4 && options[2] !== '--host-executable')
) {
	throw new Error(usage);
}

const source = realpathSync(options[1]);
if (!lstatSync(source).isFile()) throw new Error('Keeper link test executable is not a file');
const hostSource = options.length === 4 ? realpathSync(options[3]) : undefined;
if (hostSource !== undefined && !lstatSync(hostSource).isFile()) {
	throw new Error('Host executable is not a file');
}
const temporaryParent = realpathSync(os.tmpdir());
const temporary = mkdtempSync(path.join(temporaryParent, temporaryPrefix));
let failure;
try {
	// The Host's service runs in the test, so its terminals use the pinned conpty beside it.
	const executable = path.join(temporary, path.basename(source));
	copyFileSync(source, executable, constants.COPYFILE_EXCL);
	if (hostSource !== undefined) {
		// A test of moves between two Hosts starts the built Host beside it, where its terminals use
		// the same pinned conpty.
		copyFileSync(
			hostSource,
			path.join(temporary, path.basename(hostSource)),
			constants.COPYFILE_EXCL,
		);
	}
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
