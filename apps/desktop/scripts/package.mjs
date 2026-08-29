import { access, cp, lstat, mkdir, mkdtemp, readFile, rm, stat, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { packager } from '@electron/packager';
import {
	SSH_RUNTIME_RELEASE_DIRECTORY_NAME,
	SSH_RUNTIME_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
} from '@orchbee/ssh-environment-protocol';

import {
	readVerifiedSshRuntimeRelease,
	resolveSshRuntimeReleasePackageConfiguration,
	stageSshRuntimeReleasePackageResources,
} from './ssh-runtime-release-package.mjs';

const runtimeReleaseConfiguration = resolveSshRuntimeReleasePackageConfiguration(
	process.argv.slice(2),
	process.env,
);

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
const sshAskPassHelperEntry = path.join(desktopBuild, 'main', 'ssh-askpass-helper.cjs');
// Cross-application artifact contract: search for `semantic-command-cli-entry-name` before changing this value.
const semanticCommandCliEntry = path.join(
	repositoryRoot,
	'apps',
	'cli',
	'dist',
	'main',
	'orchbee-cli.cjs',
);
// Cross-application artifact contract: search for `semantic-command-cli-launcher-name` before changing these values.
const semanticCommandCliLauncher = path.join(
	repositoryRoot,
	'apps',
	'cli',
	'resources',
	process.platform === 'win32' ? 'orchbee-cli.cmd' : 'orchbee-cli',
);
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
// Cross-runtime contract: search for `posix-host-endpoint-addon-name` before changing this value.
const posixHostEndpointAddonName = 'orchbee-posix-host-endpoint.node';
const posixHostEndpointAddon = path.join(
	applicationRoot,
	'zig-out',
	'lib',
	posixHostEndpointAddonName,
);
// Cross-runtime contract: search for `semantic-command-endpoint-addon-name` before changing this value.
const semanticCommandEndpointAddonName = 'orchbee-semantic-command-endpoint.node';
const semanticCommandEndpointAddon = path.join(
	applicationRoot,
	'zig-out',
	'lib',
	semanticCommandEndpointAddonName,
);
const shellIntegrationResources = path.join(applicationRoot, 'resources', 'shell-integration');

await Promise.all([
	access(path.join(desktopBuild, 'main', 'main.cjs')),
	access(path.join(desktopBuild, 'preload', 'preload.cjs')),
	access(sshAskPassHelperEntry),
	access(semanticCommandCliEntry),
	access(semanticCommandCliLauncher),
	access(path.join(rendererBuild, 'index.html')),
	access(gatewayBuild),
	access(serverBuild),
	access(terminalHostBinary),
	...(process.platform === 'win32' ? [] : [access(posixHostEndpointAddon)]),
	access(semanticCommandEndpointAddon),
	access(path.join(shellIntegrationResources, 'orchbee.zsh')),
	access(path.join(shellIntegrationResources, 'orchbee.bash')),
	access(path.join(shellIntegrationResources, 'orchbee.fish')),
	access(path.join(shellIntegrationResources, 'automatic', 'zsh', '.zshenv')),
	access(path.join(shellIntegrationResources, 'automatic', 'bash', 'orchbee.bash')),
	access(
		path.join(
			shellIntegrationResources,
			'automatic',
			'fish',
			'fish',
			'vendor_conf.d',
			'orchbee-shell-integration.fish',
		),
	),
]);
if (process.platform !== 'win32' && ((await stat(semanticCommandCliLauncher)).mode & 0o111) === 0) {
	throw new Error('The packaged semantic command CLI launcher must be executable.');
}

const packageWorkspace = await mkdtemp(path.join(tmpdir(), 'orchbee-desktop-package-'));
const stagingRoot = path.join(packageWorkspace, 'application');
try {
	await mkdir(stagingRoot, { mode: 0o700 });
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
	const runtimeReleaseResources =
		runtimeReleaseConfiguration === null
			? null
			: await stageSshRuntimeReleasePackageResources({
					applicationStagingRoot: stagingRoot,
					externalResourcesStagingRoot: path.join(packageWorkspace, 'external-resources'),
					publicKeyFilePath: runtimeReleaseConfiguration.publicKeyFilePath,
					releaseDirectory: runtimeReleaseConfiguration.releaseDirectory,
				});
	const outputs = await packager({
		asar: true,
		dir: stagingRoot,
		electronVersion: electronPackage.version,
		extraResource: [
			rendererBuild,
			semanticCommandCliEntry,
			semanticCommandCliLauncher,
			sshAskPassHelperEntry,
			stagedGateway,
			stagedServer,
			terminalHostBinary,
			...(process.platform === 'win32' ? [] : [posixHostEndpointAddon]),
			semanticCommandEndpointAddon,
			shellIntegrationResources,
			...(runtimeReleaseResources === null
				? []
				: [runtimeReleaseResources.releaseResourceDirectory]),
		],
		name: 'Orchbee',
		out: path.join(applicationRoot, 'out'),
		overwrite: true,
		prune: false,
	});
	for (const output of outputs) {
		const resourcesDirectory =
			process.platform === 'darwin'
				? path.join(output, 'Orchbee.app', 'Contents', 'Resources')
				: path.join(output, 'resources');
		const executablePath =
			process.platform === 'darwin'
				? path.join(output, 'Orchbee.app', 'Contents', 'MacOS', 'Orchbee')
				: path.join(output, process.platform === 'win32' ? 'Orchbee.exe' : 'Orchbee');
		const packagedLauncher = path.join(
			resourcesDirectory,
			path.basename(semanticCommandCliLauncher),
		);
		await Promise.all([
			access(executablePath),
			access(path.join(resourcesDirectory, path.basename(sshAskPassHelperEntry))),
			access(path.join(resourcesDirectory, path.basename(semanticCommandCliEntry))),
			access(path.join(resourcesDirectory, path.basename(semanticCommandEndpointAddon))),
			access(packagedLauncher),
			...(runtimeReleaseResources === null
				? []
				: [access(path.join(resourcesDirectory, 'app.asar'))]),
		]);
		if (runtimeReleaseConfiguration !== null) {
			await readVerifiedSshRuntimeRelease({
				publicKeyFilePath: runtimeReleaseConfiguration.publicKeyFilePath,
				releaseDirectory: path.join(resourcesDirectory, SSH_RUNTIME_RELEASE_DIRECTORY_NAME),
			});
			await requireMissing(
				path.join(resourcesDirectory, SSH_RUNTIME_RELEASE_TRUST_DESCRIPTOR_FILE_NAME),
			);
		}
		if (process.platform !== 'win32' && ((await stat(packagedLauncher)).mode & 0o111) === 0) {
			throw new Error('The packaged semantic command CLI launcher is not executable.');
		}
		console.info(`Packaged Orchbee Desktop: ${output}`);
	}
} finally {
	await rm(packageWorkspace, { force: true, recursive: true });
}

async function requireMissing(target) {
	try {
		await lstat(target);
	} catch (error) {
		if (error instanceof Error && 'code' in error && error.code === 'ENOENT') return;
		throw error;
	}
	throw new Error(`SSH Runtime trust descriptor escaped app.asar: ${target}`);
}
