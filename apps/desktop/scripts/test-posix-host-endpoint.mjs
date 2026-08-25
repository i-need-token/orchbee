import { execFile } from 'node:child_process';
import { mkdir } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';

import { buildPosixHostEndpoint } from './build-posix-host-endpoint.mjs';

const executeFile = promisify(execFile);
if (process.platform !== 'darwin' && process.platform !== 'linux') {
	console.info('Desktop POSIX Host endpoint native tests are not applicable on this platform.');
	process.exit(0);
}

const desktopRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const nativeSourceRoot = path.join(desktopRoot, 'src', 'native', 'posix-host-endpoint');
const testOutputRoot = path.join(desktopRoot, 'zig-out', 'test');
const nativeTestBinary = path.join(testOutputRoot, 'posix-host-endpoint-test');
const addonPath = await buildPosixHostEndpoint();
if (addonPath === null) throw new Error('The POSIX Host endpoint addon was not built.');
await mkdir(testOutputRoot, { mode: 0o700, recursive: true });
await executeFile(
	'zig',
	[
		'cc',
		'-std=c17',
		'-O2',
		'-Wall',
		'-Wextra',
		'-Wpedantic',
		'-Werror=incompatible-pointer-types',
		`-I${nativeSourceRoot}`,
		path.join(nativeSourceRoot, 'posix_host_endpoint.c'),
		path.join(nativeSourceRoot, '__tests__', 'posix_host_endpoint_test.c'),
		'-o',
		nativeTestBinary,
	],
	{ encoding: 'utf8' },
);

const runtimeVerifier = path.join(desktopRoot, 'scripts', 'verify-posix-host-endpoint-runtime.mjs');
const electronExecutable = createRequire(import.meta.url)('electron');
if (typeof electronExecutable !== 'string') {
	throw new Error('The pinned Electron executable is unavailable.');
}
for (const [command, arguments_, environment] of [
	[nativeTestBinary, [], process.env],
	[process.execPath, [runtimeVerifier, addonPath], process.env],
	[electronExecutable, [runtimeVerifier, addonPath], { ...process.env, ELECTRON_RUN_AS_NODE: '1' }],
]) {
	const { stderr, stdout } = await executeFile(command, arguments_, {
		encoding: 'utf8',
		env: environment,
	});
	if (stdout.length > 0) process.stdout.write(stdout);
	if (stderr.length > 0) process.stderr.write(stderr);
}
