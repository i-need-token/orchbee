import { spawnSync } from 'node:child_process';
import { constants, copyFileSync, lstatSync, mkdtempSync, realpathSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { stageConptyBundle, verifyConptyBundle } from './windows-conpty.mjs';

const temporaryPrefix = 'orchbee-conpty-pointer-';
const packageRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = path.resolve(packageRoot, '../..');
const orchbeeSource = realpathSync(path.join(repositoryRoot, 'apps/orchbee/src/main.rs'));
const childName = 'orchbee-conpty-pointer-child.exe';
const maximumCargoOutputBytes = 64 * 1024 * 1024;
const witnessTimeoutMilliseconds = 120_000;

function requireRegularFile(candidate, description) {
	const metadata = lstatSync(candidate);
	if (!metadata.isFile() || metadata.isSymbolicLink()) {
		throw new Error(`${description} is not a regular non-symlink file`);
	}
	return realpathSync(candidate);
}

function cargoTestExecutable() {
	const result = spawnSync(
		'cargo',
		[
			'test',
			'--locked',
			'-p',
			'orchbee',
			'--bin',
			'orchbee',
			'--no-run',
			'--message-format=json-render-diagnostics',
		],
		{
			cwd: repositoryRoot,
			encoding: 'utf8',
			maxBuffer: maximumCargoOutputBytes,
			windowsHide: true,
		},
	);
	if (result.error !== undefined) throw result.error;

	const diagnostics = [];
	const candidates = new Set();
	let buildFinished = false;
	let buildSucceeded = false;
	for (const line of result.stdout.split(/\r?\n/u)) {
		if (line.length === 0) continue;
		let message;
		try {
			message = JSON.parse(line);
		} catch (error) {
			throw new Error('Cargo emitted a non-JSON line while locating the Orchbee test binary', {
				cause: error,
			});
		}
		if (message.reason === 'compiler-message') {
			const rendered = message.message?.rendered;
			if (typeof rendered === 'string') diagnostics.push(rendered);
		}
		if (message.reason === 'build-finished') {
			buildFinished = true;
			buildSucceeded = message.success === true;
		}
		if (
			message.reason === 'compiler-artifact' &&
			message.target?.name === 'orchbee' &&
			Array.isArray(message.target.kind) &&
			message.target.kind.length === 1 &&
			message.target.kind[0] === 'bin' &&
			message.profile?.test === true &&
			typeof message.target.src_path === 'string' &&
			realpathSync(message.target.src_path) === orchbeeSource &&
			typeof message.executable === 'string'
		) {
			candidates.add(requireRegularFile(message.executable, 'Orchbee Cargo test artifact'));
		}
	}

	if (result.status !== 0 || !buildFinished || !buildSucceeded) {
		const detail = [...diagnostics, result.stderr].filter(Boolean).join('\n');
		throw new Error(
			`Cargo did not build the Orchbee test binary successfully${detail.length > 0 ? `:\n${detail}` : ''}`,
		);
	}
	if (candidates.size !== 1) {
		throw new Error(`Expected one Orchbee test binary, observed ${candidates.size}`);
	}
	return candidates.values().next().value;
}

function removeOwnedTemporary(directory, parent) {
	const actual = realpathSync(directory);
	if (path.dirname(actual) !== parent || !path.basename(actual).startsWith(temporaryPrefix)) {
		throw new Error('Refusing cleanup outside owned ConPTY pointer witness directory');
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
		'Usage: run-conpty-orchbee-pointer-test.mjs --test-executable WINDOWS_X64_TEST_EXECUTABLE',
	);
}

const zigSource = requireRegularFile(process.argv[3], 'ConPTY pointer witness executable');
const rustSource = cargoTestExecutable();
const temporaryParent = realpathSync(os.tmpdir());
const temporary = mkdtempSync(path.join(temporaryParent, temporaryPrefix));
let failure;
try {
	const executable = path.join(temporary, 'terminal-host-conpty-orchbee-pointer-test.exe');
	copyFileSync(zigSource, executable, constants.COPYFILE_EXCL);
	requireRegularFile(executable, 'staged ConPTY pointer witness executable');
	const child = path.join(temporary, childName);
	copyFileSync(rustSource, child, constants.COPYFILE_EXCL);
	requireRegularFile(child, 'staged Orchbee pointer witness child');

	const bundle = path.join(temporary, 'conpty');
	await stageConptyBundle(bundle);
	verifyConptyBundle(bundle);

	const result = spawnSync(executable, [], {
		stdio: 'inherit',
		timeout: witnessTimeoutMilliseconds,
		windowsHide: true,
	});
	if (result.error !== undefined) {
		if (result.error.code === 'ETIMEDOUT') {
			throw new Error(`ConPTY pointer witness timed out after ${witnessTimeoutMilliseconds} ms`, {
				cause: result.error,
			});
		}
		throw result.error;
	}
	if (result.status !== 0) {
		throw new Error(
			`ConPTY pointer witness exited with ${
				result.signal === null ? `status ${result.status ?? 'unknown'}` : `signal ${result.signal}`
			}`,
		);
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
			'ConPTY pointer witness failed and its owned temporary directory could not be removed',
		);
	}
	throw cleanupError;
}
if (failure !== undefined) throw failure;
