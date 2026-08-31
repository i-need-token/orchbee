export function prepareDesktopDevelopmentExecutable(options: {
	readonly baseIconPath: string;
	readonly electronExecutable: string;
	readonly environment: NodeJS.ProcessEnv;
	readonly outputRoot: string;
	readonly sourceRoot: string;
}): Promise<string>;
