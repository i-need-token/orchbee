import { access, cp, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { packager } from '@electron/packager';

const electronPackage = createRequire(import.meta.url)('electron/package.json');
if (typeof electronPackage?.version !== 'string') {
	throw new Error('The pinned Electron version is unavailable.');
}

const applicationRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = path.resolve(applicationRoot, '..', '..');
const applicationPackage = JSON.parse(
	await readFile(path.join(applicationRoot, 'package.json'), 'utf8'),
);
if (
	typeof applicationPackage?.description !== 'string' ||
	typeof applicationPackage?.version !== 'string'
) {
	throw new Error('The Desktop package identity is unavailable.');
}
const desktopBuild = path.join(applicationRoot, 'dist');
const gatewayBuild = path.join(repositoryRoot, 'apps', 'gateway', 'dist', 'main', 'gateway.cjs');
const rendererBuild = path.join(repositoryRoot, 'apps', 'web', 'dist');
const serverBuild = path.join(repositoryRoot, 'apps', 'server', 'dist', 'main', 'server.cjs');
const terminalHostBinaryName =
	process.platform === 'win32' ? 'orchbee-terminal-host.exe' : 'orchbee-terminal-host';
const terminalHostBinary = path.join(
	repositoryRoot,
	'apps',
	'terminal-host',
	'zig-out',
	'bin',
	terminalHostBinaryName,
);

await Promise.all([
	access(path.join(desktopBuild, 'main', 'main.cjs')),
	access(path.join(desktopBuild, 'preload', 'preload.cjs')),
	access(path.join(rendererBuild, 'index.html')),
	access(gatewayBuild),
	access(serverBuild),
	access(terminalHostBinary),
]);

const stagingRoot = await mkdtemp(path.join(tmpdir(), 'orchbee-desktop-package-'));
try {
	await cp(desktopBuild, path.join(stagingRoot, 'dist'), { recursive: true });
	const stagedGateway = path.join(stagingRoot, 'orchbee-gateway.cjs');
	const stagedServer = path.join(stagingRoot, 'orchbee-server.cjs');
	await Promise.all([cp(gatewayBuild, stagedGateway), cp(serverBuild, stagedServer)]);
	await writeFile(
		path.join(stagingRoot, 'package.json'),
		`${JSON.stringify(
			{
				description: applicationPackage.description,
				main: 'dist/main/main.cjs',
				name: 'orchbee',
				private: true,
				version: applicationPackage.version,
			},
			null,
			2,
		)}\n`,
		'utf8',
	);
	const outputs = await packager({
		asar: true,
		dir: stagingRoot,
		electronVersion: electronPackage.version,
		extraResource: [rendererBuild, stagedGateway, stagedServer, terminalHostBinary],
		name: 'Orchbee',
		out: path.join(applicationRoot, 'out'),
		overwrite: true,
		prune: false,
	});
	for (const output of outputs) console.info(`Packaged Orchbee Desktop: ${output}`);
} finally {
	await rm(stagingRoot, { force: true, recursive: true });
}
