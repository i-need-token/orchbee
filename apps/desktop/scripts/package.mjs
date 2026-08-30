import {
	access,
	cp,
	lstat,
	mkdir,
	mkdtemp,
	readFile,
	realpath,
	rm,
	stat,
	writeFile,
} from 'node:fs/promises';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { packager } from '@electron/packager';
import {
	SSH_ENVIRONMENT_RELEASE_DIRECTORY_NAME,
	SSH_ENVIRONMENT_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
} from '@orchbee/ssh-environment-protocol';

import {
	readVerifiedSshEnvironmentRelease,
	resolveSshEnvironmentReleasePackageConfiguration,
	stageSshEnvironmentReleasePackageResources,
} from './ssh-environment-release-package.mjs';
import { materializeDarwinApplicationIcon } from './materialize-darwin-application-icon.mjs';
import {
	DESKTOP_DISTRIBUTION_DESCRIPTOR_FILE_NAME,
	createDesktopDevelopmentScope,
	encodeDesktopDistributionDescriptor,
	parseDesktopDistributionProfileArguments,
	resolveDesktopDistributionProfile,
} from '../src/distribution/desktop-distribution-profile.ts';

const electronPackage = createRequire(import.meta.url)('electron/package.json');
if (typeof electronPackage?.version !== 'string') {
	throw new Error('The pinned Electron version is unavailable.');
}

const applicationRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const repositoryRoot = path.resolve(applicationRoot, '..', '..');
const profileId = parseDesktopDistributionProfileArguments(process.argv.slice(2));
const developmentScope =
	profileId === 'dev'
		? createDesktopDevelopmentScope(await realpath(repositoryRoot), process.platform)
		: undefined;
const distributionProfile = resolveDesktopDistributionProfile(profileId, developmentScope);
const environmentReleaseConfiguration = resolveSshEnvironmentReleasePackageConfiguration(
	distributionProfile.releasePayload === 'required' ? ['--with-ssh-environment-release'] : [],
	process.env,
);
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
const agentObservationAdapterResources = path.join(
	applicationRoot,
	'resources',
	'agent-observation-adapters',
);
const baseApplicationIcon = path.join(repositoryRoot, 'resources', 'icon.svg');

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
	access(path.join(agentObservationAdapterResources, '.agents', 'plugins', 'marketplace.json')),
	access(path.join(agentObservationAdapterResources, '.claude-plugin', 'marketplace.json')),
	access(path.join(agentObservationAdapterResources, 'bundle', '.claude-plugin', 'plugin.json')),
	access(path.join(agentObservationAdapterResources, 'bundle', '.codex-plugin', 'plugin.json')),
	access(path.join(agentObservationAdapterResources, 'bundle', '.grok-plugin', 'plugin.json')),
	access(path.join(agentObservationAdapterResources, 'bundle', '.plugin', 'plugin.json')),
	access(path.join(agentObservationAdapterResources, 'bundle', 'package.json')),
	access(path.join(agentObservationAdapterResources, 'bundle', 'hooks', 'hooks.json')),
	access(path.join(agentObservationAdapterResources, 'bundle', 'hooks', 'codex.json')),
	access(path.join(agentObservationAdapterResources, 'bundle', 'hooks', 'grok.json')),
	access(path.join(agentObservationAdapterResources, 'bundle', 'hooks', 'copilot.json')),
	access(path.join(agentObservationAdapterResources, 'bundle', 'gemini', 'gemini-extension.json')),
	access(path.join(agentObservationAdapterResources, 'bundle', 'gemini', 'hooks', 'hooks.json')),
	access(path.join(agentObservationAdapterResources, 'bundle', 'qwen-code', 'qwen-extension.json')),
	access(path.join(agentObservationAdapterResources, 'bundle', 'qwen-code', 'hooks', 'hooks.json')),
	access(
		path.join(agentObservationAdapterResources, 'bundle', 'cline', 'orchbee-agent-observation.js'),
	),
	access(path.join(agentObservationAdapterResources, 'bundle', 'scripts', 'forward-hook.mjs')),
	access(path.join(agentObservationAdapterResources, 'bundle', 'scripts', 'hook-sanitizers.mjs')),
	access(
		path.join(agentObservationAdapterResources, 'bundle', 'scripts', 'observation-transport.mjs'),
	),
	access(
		path.join(
			agentObservationAdapterResources,
			'bundle',
			'opencode',
			'orchbee-agent-observation.mjs',
		),
	),
	access(
		path.join(agentObservationAdapterResources, 'bundle', 'pi', 'orchbee-agent-observation.mjs'),
	),
	access(
		path.join(agentObservationAdapterResources, 'bundle', 'pi', 'orchbee-agent-observation.cjs'),
	),
	access(
		path.join(agentObservationAdapterResources, 'bundle', 'omp', 'orchbee-agent-observation.mjs'),
	),
	access(
		path.join(agentObservationAdapterResources, 'bundle', 'omp', 'orchbee-agent-observation.cjs'),
	),
	access(baseApplicationIcon),
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
				name: distributionProfile.packageName,
				private: true,
				version: applicationPackage.version,
			},
			null,
			2,
		)}\n`,
		'utf8',
	);
	await writeFile(
		path.join(stagingRoot, DESKTOP_DISTRIBUTION_DESCRIPTOR_FILE_NAME),
		encodeDesktopDistributionDescriptor(distributionProfile),
		{ encoding: 'utf8', mode: 0o400 },
	);
	const environmentReleaseResources =
		environmentReleaseConfiguration === null
			? null
			: await stageSshEnvironmentReleasePackageResources({
					applicationStagingRoot: stagingRoot,
					externalResourcesStagingRoot: path.join(packageWorkspace, 'external-resources'),
					publicKeyFilePath: environmentReleaseConfiguration.publicKeyFilePath,
					releaseDirectory: environmentReleaseConfiguration.releaseDirectory,
				});
	const applicationIcon =
		process.platform === 'darwin'
			? await materializeDarwinApplicationIcon({
					baseIconSvg: await readFile(baseApplicationIcon, 'utf8'),
					outputDirectory: path.join(packageWorkspace, 'application-icon'),
					profile: distributionProfile,
				})
			: undefined;
	const outputs = await packager({
		appBundleId: distributionProfile.bundleIdentifier,
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
			agentObservationAdapterResources,
			...(environmentReleaseResources === null
				? []
				: [environmentReleaseResources.releaseResourceDirectory]),
		],
		helperBundleId: `${distributionProfile.bundleIdentifier}.helper`,
		...(applicationIcon === undefined ? {} : { icon: applicationIcon }),
		name: distributionProfile.applicationName,
		out: path.join(applicationRoot, 'out', distributionProfile.outputDirectoryName),
		overwrite: true,
		prune: false,
	});
	for (const output of outputs) {
		const applicationFileName = `${distributionProfile.applicationName}.app`;
		const resourcesDirectory =
			process.platform === 'darwin'
				? path.join(output, applicationFileName, 'Contents', 'Resources')
				: path.join(output, 'resources');
		const executablePath =
			process.platform === 'darwin'
				? path.join(
						output,
						applicationFileName,
						'Contents',
						'MacOS',
						distributionProfile.applicationName,
					)
				: path.join(
						output,
						process.platform === 'win32'
							? `${distributionProfile.applicationName}.exe`
							: distributionProfile.applicationName,
					);
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
			...(environmentReleaseResources === null
				? []
				: [access(path.join(resourcesDirectory, 'app.asar'))]),
		]);
		if (environmentReleaseConfiguration !== null) {
			await readVerifiedSshEnvironmentRelease({
				publicKeyFilePath: environmentReleaseConfiguration.publicKeyFilePath,
				releaseDirectory: path.join(resourcesDirectory, SSH_ENVIRONMENT_RELEASE_DIRECTORY_NAME),
			});
			await requireMissing(
				path.join(resourcesDirectory, SSH_ENVIRONMENT_RELEASE_TRUST_DESCRIPTOR_FILE_NAME),
			);
		}
		if (process.platform !== 'win32' && ((await stat(packagedLauncher)).mode & 0o111) === 0) {
			throw new Error('The packaged semantic command CLI launcher is not executable.');
		}
		console.info(`Packaged Orchbee Desktop (${distributionProfile.profileId}): ${output}`);
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
	throw new Error(`SSH Environment trust descriptor escaped app.asar: ${target}`);
}
