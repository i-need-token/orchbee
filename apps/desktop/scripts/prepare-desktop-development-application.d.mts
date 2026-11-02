export function prepareDesktopDevelopmentExecutable(options: {
	readonly electronExecutable: string;
	readonly environment: NodeJS.ProcessEnv;
	readonly iconDirectory: string;
	readonly outputRoot: string;
	readonly sourceRoot: string;
}): Promise<string>;
