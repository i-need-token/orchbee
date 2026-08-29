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
	if (
		typeof nodeApiHeaders?.include_dir !== 'string' ||
		typeof nodeApiHeaders?.def_paths?.node_api_def !== 'string'
	) {
		throw new Error('The pinned Node-API headers are unavailable.');
	}
	const { stdout } = await executeFile('zig', ['version'], { encoding: 'utf8' });
	if (stdout.trim() !== '0.16.0') {
		throw new Error(
			`Desktop semantic command endpoint requires Zig 0.16.0, received ${stdout.trim()}.`,
		);
	}
	await mkdir(nativeOutputRoot, { mode: 0o700, recursive: true });
	let windowsImportLibrary;
	if (process.platform === 'win32') {
		const machine = windowsImportLibraryMachine(process.arch);
		windowsImportLibrary = path.join(nativeOutputRoot, 'node-api.lib');
		await executeFile(
			'zig',
			[
				'dlltool',
				'-d',
				nodeApiHeaders.def_paths.node_api_def,
				'-l',
				windowsImportLibrary,
				'-m',
				machine,
			],
			{ encoding: 'utf8' },
		);
	}
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
		`-I${nodeApiHeaders.include_dir}`,
		path.join(nativeSourceRoot, platformSource),
		path.join(nativeSourceRoot, 'semantic_command_endpoint_addon.c'),
		...(windowsImportLibrary === undefined ? [] : [windowsImportLibrary]),
		'-o',
		semanticCommandEndpointAddonPath,
		...(process.platform === 'darwin' ? ['-Wl,-undefined,dynamic_lookup'] : []),
	];
	await executeFile('zig', arguments_, { encoding: 'utf8' });
	return semanticCommandEndpointAddonPath;
}

function windowsImportLibraryMachine(architecture) {
	if (architecture === 'x64') return 'i386:x86-64';
	if (architecture === 'arm64') return 'arm64';
	return 'i386';
}

if (process.argv[1] === fileURLToPath(import.meta.url)) {
	const output = await buildSemanticCommandEndpoint();
	console.info(`Built Desktop semantic command endpoint: ${output}`);
}
