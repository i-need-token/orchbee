import { access, cp, mkdir, mkdtemp, readFile, realpath, rm, writeFile } from 'node:fs/promises';
import { createRequire } from 'node:module';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

import { packager } from '@electron/packager';

import {
	DESKTOP_DISTRIBUTION_DESCRIPTOR_FILE_NAME,
	createDesktopDevelopmentScope,
	resolveDesktopDevelopmentApplicationName,
	resolveDesktopDistributionProfile,
} from '../src/distribution/desktop-distribution-profile.ts';
import {
	encodeDesktopDistributionDescriptor,
	parseDesktopDistributionProfileArguments,
	resolveDesktopLocalApplicationName,
} from '../src/distribution/tooling/index.ts';

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
const distributionProfile = (() => {
	if (profileId === 'dev') {
		return resolveDesktopDistributionProfile(profileId, developmentScope, {
			applicationName: resolveDesktopDevelopmentApplicationName(process.env),
		});
	}
	if (profileId === 'local') {
		return resolveDesktopDistributionProfile(profileId, undefined, {
			applicationName: resolveDesktopLocalApplicationName(process.env),
		});
	}
	return resolveDesktopDistributionProfile(profileId);
})();
const usesDarwinApplicationIcon = process.platform === 'darwin';
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
const rendererBuild = path.join(repositoryRoot, 'apps', 'web', 'dist');
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
const shellIntegrationResources = path.join(applicationRoot, 'resources', 'shell-integration');
const agentObservationAdapterResources = path.join(
	applicationRoot,
	'resources',
	'agent-observation-adapters',
);
const darwinApplicationIcon = path.join(
	repositoryRoot,
	'resources',
	'brand',
	'icns',
	distributionProfile.iconFileName,
);

await Promise.all([
	access(path.join(desktopBuild, 'main', 'main.cjs')),
	access(path.join(desktopBuild, 'preload', 'preload.cjs')),
	access(path.join(rendererBuild, 'index.html')),
	access(terminalHostBinary),
	...(process.platform === 'win32' ? [] : [access(posixHostEndpointAddon)]),
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
	...(usesDarwinApplicationIcon ? [access(darwinApplicationIcon)] : []),
]);

const packageWorkspace = await mkdtemp(path.join(tmpdir(), 'orchbee-desktop-package-'));
const stagingRoot = path.join(packageWorkspace, 'application');
try {
	await mkdir(stagingRoot, { mode: 0o700 });
	await cp(desktopBuild, path.join(stagingRoot, 'dist'), { recursive: true });
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
	const outputs = await packager({
		appBundleId: distributionProfile.bundleIdentifier,
		asar: true,
		dir: stagingRoot,
		electronVersion: electronPackage.version,
		extraResource: [
			path.join(applicationRoot, 'out', 'settings-bridge'),
			rendererBuild,
			terminalHostBinary,
			...(process.platform === 'win32' ? [] : [posixHostEndpointAddon]),
			shellIntegrationResources,
			agentObservationAdapterResources,
		],
		helperBundleId: `${distributionProfile.bundleIdentifier}.helper`,
		...(usesDarwinApplicationIcon ? { icon: darwinApplicationIcon } : {}),
		name: distributionProfile.applicationName,
		out: path.join(applicationRoot, 'out', distributionProfile.outputDirectoryName),
		overwrite: true,
		prune: false,
	});
	for (const output of outputs) {
		const applicationFileName = `${distributionProfile.applicationName}.app`;
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
		await access(executablePath);
		console.info(`Packaged Orchbee Desktop (${distributionProfile.profileId}): ${output}`);
	}
} finally {
	await rm(packageWorkspace, { force: true, recursive: true });
}
