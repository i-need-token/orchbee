/** The part of Electron's main `process` that tells how the application was started. */
interface DesktopLaunch {
	/** Set by Electron's default app when it loads an application path given on the command line. */
	readonly defaultApp?: boolean;
}

/**
 * Whether this Main runs as a packaged Desktop rather than a checkout's entry: development always
 * starts through Electron's default app (`electron <entry>`), which a packaged Desktop never ships,
 * so the executable's name decides nothing and no environment variable can turn a packaged Desktop
 * into a development one. `app.isPackaged` cannot answer this: it only compares the executable's
 * name with `electron`, and the Windows development shell carries a packaged-style name.
 */
export function isPackagedDesktop(launch: DesktopLaunch): boolean {
	return launch.defaultApp !== true;
}
