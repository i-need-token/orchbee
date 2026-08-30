import {
	DESKTOP_DEVELOPMENT_SSH_ENVIRONMENT_RELEASE_DIRECTORY_ENVIRONMENT_KEY,
	DESKTOP_DEVELOPMENT_SSH_ENVIRONMENT_TRUST_DESCRIPTOR_ENVIRONMENT_KEY,
} from '../../development';

export function resolveDesktopDevelopmentSshEnvironmentRelease(
	environment: Readonly<Record<string, string | undefined>>,
):
	| {
			readonly releaseDirectoryPath: string;
			readonly trustDescriptorFilePath: string;
	  }
	| undefined {
	const releaseDirectoryPath =
		environment[DESKTOP_DEVELOPMENT_SSH_ENVIRONMENT_RELEASE_DIRECTORY_ENVIRONMENT_KEY];
	const trustDescriptorFilePath =
		environment[DESKTOP_DEVELOPMENT_SSH_ENVIRONMENT_TRUST_DESCRIPTOR_ENVIRONMENT_KEY];
	if (releaseDirectoryPath === undefined && trustDescriptorFilePath === undefined) return undefined;
	if (releaseDirectoryPath === undefined || trustDescriptorFilePath === undefined) {
		throw new Error('Development SSH Environment release configuration is incomplete.');
	}
	return { releaseDirectoryPath, trustDescriptorFilePath };
}
