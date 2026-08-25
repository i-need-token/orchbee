import { execFile } from 'node:child_process';
import { mkdir } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';

const executeFile = promisify(execFile);
const desktopRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const nativeSourceRoot = path.join(desktopRoot, 'src', 'native', 'posix-host-endpoint');
const nativeOutputRoot = path.join(desktopRoot, 'zig-out', 'lib');
// Cross-runtime contract: search for `posix-host-endpoint-addon-name` before changing this value.
const posixHostEndpointAddonName = 'orchbee-posix-host-endpoint.node';
const posixHostEndpointAddonPath = path.join(nativeOutputRoot, posixHostEndpointAddonName);

export async function buildPosixHostEndpoint() {
	if (process.platform !== 'darwin' && process.platform !== 'linux') return null;
	const nodeApiHeaders = createRequire(import.meta.url)('node-api-headers');
	if (typeof nodeApiHeaders?.include_dir !== 'string') {
		throw new Error('The pinned Node-API include directory is unavailable.');
	}
	const { stdout } = await executeFile('zig', ['version'], { encoding: 'utf8' });
	if (stdout.trim() !== '0.15.2') {
		throw new Error(`Desktop native endpoint requires Zig 0.15.2, received ${stdout.trim()}.`);
	}
	await mkdir(nativeOutputRoot, { mode: 0o700, recursive: true });
	const arguments_ = [
		'cc',
		'-std=c17',
		'-O2',
		'-fPIC',
		'-shared',
		'-Wall',
		'-Wextra',
		'-Wpedantic',
		'-Werror=incompatible-pointer-types',
		'-DNAPI_VERSION=8',
		`-I${nodeApiHeaders.include_dir}`,
		path.join(nativeSourceRoot, 'posix_host_endpoint.c'),
		path.join(nativeSourceRoot, 'posix_host_endpoint_addon.c'),
		'-o',
		posixHostEndpointAddonPath,
		...(process.platform === 'darwin' ? ['-Wl,-undefined,dynamic_lookup'] : []),
	];
	await executeFile('zig', arguments_, { encoding: 'utf8' });
	return posixHostEndpointAddonPath;
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
	const output = await buildPosixHostEndpoint();
	if (output !== null) console.info(`Built Desktop POSIX Host endpoint: ${output}`);
}
