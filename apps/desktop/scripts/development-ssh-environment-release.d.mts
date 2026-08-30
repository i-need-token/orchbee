export interface DevelopmentSshEnvironmentRelease {
	readonly cohortInputDigest: string;
	readonly publicKeyFilePath: string;
	readonly releaseDirectoryPath: string;
	readonly runtimeVersion: string;
	readonly terminalHostVersion: string;
	readonly trustDescriptorFilePath: string;
}

export function prepareDevelopmentSshEnvironmentRelease(options: {
	readonly outputRoot: string;
	readonly sourceRoot: string;
}): Promise<DevelopmentSshEnvironmentRelease>;
