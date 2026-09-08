import { execFile } from 'node:child_process';
import { mkdir } from 'node:fs/promises';
import { createRequire } from 'node:module';
import path from 'node:path';
import { promisify } from 'node:util';
import { fileURLToPath } from 'node:url';

const executeFile = promisify(execFile);
const desktopRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const nativeSourceRoot = path.join(desktopRoot, 'src', 'native', 'semantic-command-endpoint');
const nativeOutputRoot = path.join(desktopRoot, 'zig-out', 'lib');
// Cross-runtime contract: search for `semantic-command-endpoint-addon-name` before changing this value.
const semanticCommandEndpointAddonName = 'orchbee-semantic-command-endpoint.node';
const semanticCommandEndpointAddonPath = path.join(
	nativeOutputRoot,
	semanticCommandEndpointAddonName,
);

export async function buildSemanticCommandEndpoint() {
	const nodeApiHeaders = createRequire(import.meta.url)('node-api-headers');
	if (typeof nodeApiHeaders?.include_dir !== 'string') {
		throw new Error('The pinned Node-API headers are unavailable.');
	}
	const { stdout } = await executeFile('zig', ['version'], { encoding: 'utf8' });
	if (stdout.trim() !== '0.16.0') {
		throw new Error(
			`Desktop semantic command endpoint requires Zig 0.16.0, received ${stdout.trim()}.`,
		);
	}
	await mkdir(nativeOutputRoot, { mode: 0o700, recursive: true });
	const platformSource =
		process.platform === 'win32'
			? 'semantic_command_endpoint_windows.c'
			: 'semantic_command_endpoint_posix.c';
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
		...(process.platform === 'win32'
			? ['-target', windowsNodeApiTarget(process.arch), '-DBUILDING_NODE_EXTENSION']
			: []),
		`-I${nodeApiHeaders.include_dir}`,
		path.join(nativeSourceRoot, platformSource),
		path.join(nativeSourceRoot, 'semantic_command_endpoint_addon.c'),
		...(process.platform === 'win32'
			? [path.join(nativeSourceRoot, 'node_api_windows_host.c')]
			: []),
		'-o',
		semanticCommandEndpointAddonPath,
		...(process.platform === 'darwin' ? ['-Wl,-undefined,dynamic_lookup'] : []),
	];
	await executeFile('zig', arguments_, { encoding: 'utf8' });
	return semanticCommandEndpointAddonPath;
}

export function windowsNodeApiTarget(architecture) {
	if (architecture === 'x64') return 'x86_64-windows-gnu';
	if (architecture === 'arm64') return 'aarch64-windows-gnu';
	if (architecture === 'ia32') return 'x86-windows-gnu';
	throw new Error(`Unsupported Windows Node-API architecture: ${architecture}.`);
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
	const output = await buildSemanticCommandEndpoint();
	console.info(`Built Desktop semantic command endpoint: ${output}`);
}
