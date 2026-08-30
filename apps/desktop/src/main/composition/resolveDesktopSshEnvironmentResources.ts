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
			readonly release?:
				| {
						readonly releaseDirectoryPath: string;
						readonly trustDescriptorFilePath: string;
				  }
				| undefined;
			readonly runtimeExecutablePath: string;
	  }
	| {
			readonly mainDirectoryPath: string;
			readonly packaged: true;
			readonly releaseAvailable: boolean;
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
		if (
			input.release !== undefined &&
			(!isAbsolute(input.release.releaseDirectoryPath) ||
				!isAbsolute(input.release.trustDescriptorFilePath))
		) {
			throw new Error('Desktop SSH Environment development release paths must be absolute.');
		}
		return {
			askPassHelperEntryPath: join(input.mainDirectoryPath, 'ssh-askpass-helper.cjs'),
			askPassRuntimeExecutablePath: input.runtimeExecutablePath,
			release: input.release ?? null,
		};
	}
	if (!isAbsolute(input.resourcesPath)) {
		throw new Error('Desktop SSH Environment packaged Resources path must be absolute.');
	}
	return {
		askPassHelperEntryPath: join(input.resourcesPath, 'ssh-askpass-helper.cjs'),
		askPassRuntimeExecutablePath: input.runtimeExecutablePath,
		release: input.releaseAvailable
			? {
					releaseDirectoryPath: join(input.resourcesPath, SSH_ENVIRONMENT_RELEASE_DIRECTORY_NAME),
					trustDescriptorFilePath: join(
						input.mainDirectoryPath,
						SSH_ENVIRONMENT_RELEASE_TRUST_DESCRIPTOR_FILE_NAME,
					),
				}
			: null,
	};
}
