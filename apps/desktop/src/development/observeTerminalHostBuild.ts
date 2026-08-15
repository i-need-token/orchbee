import type { DesktopDevelopmentHostBuild } from './DesktopDevelopmentRuntime';

interface TerminalHostBuildObservationOptions {
	readonly binaryPath: string;
	readonly currentDigest: string;
	readonly onBuild: (build: DesktopDevelopmentHostBuild) => Promise<void>;
	readonly readBinaryDigest: (binaryPath: string) => Promise<string>;
	readonly readInputDigest: () => Promise<string>;
}

/** @internal Package-private observation step split from the process and filesystem watcher. */
export async function observeTerminalHostBuild(
	options: TerminalHostBuildObservationOptions,
): Promise<string> {
	const digest = await options.readBinaryDigest(options.binaryPath);
	if (digest === options.currentDigest) return options.currentDigest;
	const inputDigest = await options.readInputDigest();
	await options.onBuild({ binaryPath: options.binaryPath, digest, inputDigest });
	return digest;
}
