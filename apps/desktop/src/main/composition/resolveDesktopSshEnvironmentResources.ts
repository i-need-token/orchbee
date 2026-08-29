import { isAbsolute, join } from 'node:path';

import {
	SSH_ENVIRONMENT_RELEASE_DIRECTORY_NAME,
	SSH_ENVIRONMENT_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
} from '@orchbee/ssh-environment-protocol';

import type { DesktopSshEnvironmentResources } from './DesktopApplication';

type DesktopSshEnvironmentResourceInput =
	| {
			readonly mainDirectoryPath: string;
			readonly packaged: false;
			readonly runtimeExecutablePath: string;
	  }
	| {
			readonly mainDirectoryPath: string;
			readonly packaged: true;
			readonly resourcesPath: string;
			readonly runtimeExecutablePath: string;
	  };

export function resolveDesktopSshEnvironmentResources(
	input: DesktopSshEnvironmentResourceInput,
): DesktopSshEnvironmentResources {
	if (!isAbsolute(input.mainDirectoryPath) || !isAbsolute(input.runtimeExecutablePath)) {
		throw new Error('Desktop SSH Environment runtime resource paths must be absolute.');
	}
	if (!input.packaged) {
		return {
			askPassHelperEntryPath: join(input.mainDirectoryPath, 'ssh-askpass-helper.cjs'),
			askPassRuntimeExecutablePath: input.runtimeExecutablePath,
			release: null,
		};
	}
	if (!isAbsolute(input.resourcesPath)) {
		throw new Error('Desktop SSH Environment packaged Resources path must be absolute.');
	}
	return {
		askPassHelperEntryPath: join(input.resourcesPath, 'ssh-askpass-helper.cjs'),
		askPassRuntimeExecutablePath: input.runtimeExecutablePath,
		release: {
			releaseDirectoryPath: join(input.resourcesPath, SSH_ENVIRONMENT_RELEASE_DIRECTORY_NAME),
			trustDescriptorFilePath: join(
				input.mainDirectoryPath,
				SSH_ENVIRONMENT_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
			),
		},
	};
}
