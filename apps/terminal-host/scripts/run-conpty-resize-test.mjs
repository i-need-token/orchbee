import { spawnSync } from 'node:child_process';
import { copyFileSync, lstatSync, mkdtempSync, realpathSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { stageConptyBundle, verifyConptyBundle } from './windows-conpty.mjs';

const temporaryPrefix = 'orchbee-conpty-resize-';

function removeOwnedTemporary(directory, parent) {
	const actual = realpathSync(directory);
	if (path.dirname(actual) !== parent || !path.basename(actual).startsWith(temporaryPrefix)) {
		throw new Error('Refusing cleanup outside owned ConPTY resize test directory');
	}
	rmSync(actual, { recursive: true });
}

if (
	process.platform !== 'win32' ||
	process.arch !== 'x64' ||
	process.argv.length !== 4 ||
	process.argv[2] !== '--test-executable'
) {
	throw new Error(
		'Usage: run-conpty-resize-test.mjs --test-executable WINDOWS_X64_TEST_EXECUTABLE',
	);
}

const source = realpathSync(process.argv[3]);
if (!lstatSync(source).isFile()) throw new Error('ConPTY resize test executable is not a file');
const temporaryParent = realpathSync(os.tmpdir());
const temporary = mkdtempSync(path.join(temporaryParent, temporaryPrefix));
try {
	const executable = path.join(temporary, 'terminal-host-conpty-resize-test.exe');
	copyFileSync(source, executable);
	const bundle = path.join(temporary, 'conpty');
	await stageConptyBundle(bundle);
	verifyConptyBundle(bundle);

	const result = spawnSync(executable, [], {
		stdio: 'inherit',
		timeout: 90_000,
		windowsHide: true,
	});
	if (result.error !== undefined) throw result.error;
	if (result.status !== 0) {
		throw new Error(`ConPTY resize test exited with status ${result.status ?? 'unknown'}`);
	}
} finally {
	removeOwnedTemporary(temporary, temporaryParent);
}
