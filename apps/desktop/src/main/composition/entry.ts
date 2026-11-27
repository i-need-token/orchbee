/**
 * The Desktop application as a Server Client: one window whose document holds one Client connection
 * through an `orchbee server connect` relay.
 */
import { spawn } from 'node:child_process';
import path from 'node:path';

import {
	createOrchbeeIntl,
	desktopDiagnosticsMessages,
	desktopWindowMessages,
	type IntlShape,
	resolveOrchbeeLocale,
	sessionPathRefusalMessages,
} from '@orchbee/i18n';
import { app, dialog, net, protocol } from 'electron';

import { DesktopDevelopmentRestartRequestSchema } from '../../development';
import { resolveDesktopCheckoutName, resolveDesktopSessionName } from '../../distribution';
import { installDesktopApplicationMenu } from '../application-menu';
import {
	DesktopBackgroundRuntime,
	DesktopBackgroundTray,
	FileDesktopTrayRecord,
} from '../background-tray';
import {
	containListenerFault,
	containMainProcessFaults,
	DesktopDiagnosticLog,
	DesktopDiagnosticsActions,
	DesktopDiagnosticsRuntime,
	type DesktopDiagnosticRecorder,
} from '../diagnostics';
import {
	DESKTOP_RENDERER_URL,
	installDesktopRendererScheme,
	isPermittedDesktopRendererUrl,
	registerDesktopRendererScheme,
} from '../desktop-renderer-protocol';
import {
	DesktopSecureKeyboardEntry,
	DesktopSecureKeyboardEntryActions,
} from '../secure-keyboard-entry';
import {
	readServerStartAnswer,
	readServerStopStatus,
	restartForUpdate,
} from '../server-connection';
import { createUpgradeBackup } from '../upgrade-backup';
import { DesktopWindowActions } from '../window-actions';
import {
	DesktopWindowStateOwner,
	type DesktopWindowStateRecordReporter,
	FileDesktopWindowStateStore,
} from '../window-state';
import {
	DesktopWindowUpdate,
	discardSetupLaunch,
	givesWayToReplacedVersion,
	locateWindowDesktopRoot,
	readFallbackFromVersion,
	readRelaunchPlace,
	readWindowPointer,
	relaunchWindow,
	resolvePosixReleaseStoreRoot,
	takeRelaunchAnnouncement,
	takeSetupLaunch,
	watchWindowPointer,
	windowVersionResources,
} from '../window-update';
import { DesktopMachinesProcess } from '../remote-machines';
import { createDesktopFolder, DesktopFolderNotCreated } from './desktop-folder';
import { DesktopDataRootRefusal } from './data-root-version-record';
import { DesktopServerClientApplication } from './DesktopServerClientApplication';
import { isPackagedDesktop } from './isPackagedDesktop';
import { type DesktopDataRoot, prepareDesktopDataRoot } from './prepareDesktopDataRoot';
import { resolveDesktopApplicationDistribution } from './resolveDesktopApplicationDistribution';

const CLIENT_EXECUTABLE_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_CLIENT_EXECUTABLE';
const DEBUG_PORT_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_DEBUG_PORT';
const RENDERER_ROOT_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_RENDERER_ROOT';
const RENDERER_URL_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_RENDERER_URL';
const SESSION_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_SESSION';
const WINDOW_HIDDEN_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_WINDOW_HIDDEN';
/** What the Windows taskbar's Save Diagnostic Bundle task starts Orchbee with. */
const SAVE_DIAGNOSTIC_BUNDLE_SWITCH = '--save-diagnostic-bundle';
/**
 * How long after startup Main runs `--tend-window-store` so any replaced window version or
 * moved-aside launcher has finished exiting and released its files.
 */
const WINDOW_STORE_TEND_DELAY_MILLISECONDS = 15_000;
// Cross-runtime contract: search for `packaged-desktop-resources` before changing these values.
const PACKAGED_COHORT_DIRECTORY = 'cohort';
const PACKAGED_RENDERER_DIRECTORY = 'dist';
const PACKAGED_TRAY_ICON_DIRECTORY = 'tray';

const launchedAt = performance.now();
// Before anything starts a process, so none inherits the launcher protocol variable.
const relaunchAnnounced = takeRelaunchAnnouncement(process.env, process.ppid);
const packaged = isPackagedDesktop(process);
/**
 * This window's product version. Electron's default app, which loads every development entry,
 * reports Electron's own version, so development takes the version this package was built with.
 */
const ownVersion = packaged ? app.getVersion() : ORCHBEE_DESKTOP_PACKAGE_VERSION;
registerDesktopRendererScheme(protocol);
const distribution = resolveDesktopApplicationDistribution({
	applicationPath: app.getAppPath(),
	environment: process.env,
	packaged,
	platform: process.platform,
});
/**
 * Development and test builds take a Session name, a hidden window and a DevTools port from the
 * environment for automation; a packaged release ignores all three, so `ORCHBEE_SESSION` stays its
 * public Session choice and no release opens a debugging port.
 */
const automationEnvironment: NodeJS.ProcessEnv =
	packaged && distribution.dataScope.kind === 'release' ? {} : process.env;
const windowHidden = automationEnvironment[WINDOW_HIDDEN_ENVIRONMENT_KEY] !== undefined;
const debugPort = readDebugPort(automationEnvironment[DEBUG_PORT_ENVIRONMENT_KEY]);
// Chromium listens on the loopback interface only; port 0 takes a free port and writes it to
// `DevToolsActivePort` in the Electron folder, which the release workflow's real-window check reads.
if (debugPort !== null) app.commandLine.appendSwitch('remote-debugging-port', debugPort);
/**
 * The store's `desktop` folder, never for a development build: on Windows and Linux when this window
 * runs from `desktop\versions\<its own version>\`, whose window entry starts whichever window version
 * is active; on macOS when it runs from an application bundle outside the store, and on Linux from
 * anywhere outside the store, as the AppImage and the Debian package do.
 */
const windowDesktopRoot = locateWindowDesktopRoot({
	development: !packaged || distribution.dataScope.kind === 'development',
	executablePath: process.execPath,
	ownVersion,
	platform: process.platform,
	storeRoot:
		process.platform === 'win32'
			? null
			: resolvePosixReleaseStoreRoot(process.env, distribution.artifactStem),
});
/** What starts this window's program: the Windows window entry when there is one, else the program. */
const windowStartProgram =
	windowDesktopRoot === null || process.platform !== 'win32'
		? process.execPath
		: path.join(windowDesktopRoot, path.basename(process.execPath));
app.setName(distribution.applicationName);
// The installer's Start menu shortcut carries the same identity, so Windows groups the window,
// its taskbar button, its Jump List and its notifications under that shortcut; a checkout's
// Electron has no shortcut of its own and keeps Electron's identity.
if (process.platform === 'win32' && packaged) {
	app.setAppUserModelId(distribution.bundleIdentifier);
}
const dataRoot = prepareOrRefuseDesktopDataRoot();
app.setPath('userData', dataRoot.electronDirectory);
const recordEvent: DesktopDiagnosticRecorder = new DesktopDiagnosticLog(
	dataRoot.logDirectory,
).recorder();

/**
 * Prepares the data root, or, when its layout is one this version cannot open or the bundled Client
 * would not create its folders, explains why and exits before anything reads or writes it.
 */
function prepareOrRefuseDesktopDataRoot(): DesktopDataRoot {
	try {
		return prepareDesktopDataRoot(
			{
				applicationVersion: ownVersion,
				distribution,
				homeDirectory: app.getPath('home'),
				homeOverride: process.env['ORCHBEE_HOME'],
				platform: process.platform,
			},
			(root) =>
				createDesktopFolder(
					{
						environment: resolveClientEnvironment(),
						executable: resolveClientExecutable(),
						platform: process.platform,
						workingDirectory: app.getPath('home'),
					},
					root,
				),
		);
	} catch (error: unknown) {
		if (!(error instanceof DesktopDataRootRefusal || error instanceof DesktopFolderNotCreated)) {
			throw error;
		}
		const intl = createOrchbeeIntl(resolveOrchbeeLocale(app.getPreferredSystemLanguages()));
		dialog.showErrorBox(
			intl.formatMessage(desktopWindowMessages.dataFolderRefused),
			describeDataFolderRefusal(intl, error),
		);
		app.exit(1);
		throw error;
	}
}

/**
 * Why the data root will not open: a layout this version cannot read, or the item the bundled
 * Client refused, in the words the TUI and CLI use.
 */
function describeDataFolderRefusal(
	intl: IntlShape,
	error: DesktopDataRootRefusal | DesktopFolderNotCreated,
): string {
	if (error instanceof DesktopFolderNotCreated) {
		if (error.refusal === undefined) {
			return intl.formatMessage(desktopWindowMessages.dataFolderNotCreated, { root: error.root });
		}
		return intl.formatMessage(sessionPathRefusalMessages[error.refusal.cause], {
			// A dialog holds plain text, so a command is quoted as the CLI quotes it.
			command: (chunks) => `"${chunks.join('')}"`,
			path: error.refusal.path,
			system: error.refusal.system,
		});
	}
	if (error.layout === undefined) {
		return intl.formatMessage(desktopWindowMessages.dataFolderUnreadable, {
			record: error.recordPath,
			root: error.root,
		});
	}
	if (error.newestVersion === undefined) {
		return intl.formatMessage(desktopWindowMessages.dataFolderLayoutNewerUnnamed, {
			root: error.root,
		});
	}
	return intl.formatMessage(desktopWindowMessages.dataFolderLayoutNewer, {
		root: error.root,
		version: error.newestVersion,
	});
}

/** Shows and logs, once, that the window placement record is one this version cannot use. */
const reportWindowStateRecord: DesktopWindowStateRecordReporter = (problem, version) => {
	if (problem === 'newer-format') {
		recordEvent('warning', 'record-unusable', {
			problem: 'newer-format',
			record: 'desktop-window',
			version,
		});
	} else {
		recordEvent('warning', 'record-unusable', {
			problem: 'not-updated',
			record: 'desktop-window',
			version,
		});
	}
	const intl = createOrchbeeIntl(resolveOrchbeeLocale(app.getPreferredSystemLanguages()));
	void dialog
		.showMessageBox({
			buttons: [intl.formatMessage(desktopWindowMessages.recordNoticeClose)],
			message: intl.formatMessage(
				problem === 'newer-format'
					? desktopWindowMessages.placementNewer
					: desktopWindowMessages.placementNotUpdated,
			),
			type: 'info',
		})
		.catch(() => undefined);
};

/**
 * What `orchbee` runs under: Desktop's own environment. The natives are compiled with the profile's
 * stem and derive the same data root, and an inherited `ORCHBEE_HOME` names it for both, so a
 * Session it starts keeps this root's configuration and preferences rather than another
 * environment's.
 */
function resolveClientEnvironment(): NodeJS.ProcessEnv {
	return process.env;
}

function requiredEnvironment(key: string): string {
	const value = process.env[key];
	if (value === undefined || value.length === 0) throw new Error(`${key} is required.`);
	return value;
}

/** The DevTools port a value names: 0, or 1024 to 65535; null for anything else. */
function readDebugPort(value: string | undefined): string | null {
	if (value === undefined || !/^\d{1,5}$/u.test(value)) return null;
	const port = Number(value);
	return port === 0 || (port >= 1024 && port <= 65_535) ? String(port) : null;
}

/** The bundled cohort's Client, or in a checkout the one its development runtime staged. */
function resolveClientExecutable(): string {
	if (!packaged) return requiredEnvironment(CLIENT_EXECUTABLE_ENVIRONMENT_KEY);
	const stem = distribution.artifactStem;
	return path.join(
		process.resourcesPath,
		PACKAGED_COHORT_DIRECTORY,
		process.platform === 'win32' ? `${stem}.exe` : stem,
	);
}

/**
 * Starts `program` outside this process's Job, so it outlives this process; resolves whether it
 * started.
 */
function startDetached(
	program: string,
	argumentsList: readonly string[],
	workingDirectory: string,
): Promise<boolean> {
	return new Promise((resolve) => {
		const child = spawn(program, argumentsList, {
			cwd: workingDirectory,
			detached: true,
			env: process.env,
			stdio: 'ignore',
		});
		child.once('error', () => resolve(false));
		child.once('spawn', () => {
			child.unref();
			resolve(true);
		});
	});
}

/**
 * The window's Update control: the newer window version its pointer names, offered only to a window
 * that runs from the store's window layout (`windowDesktopRoot`) and never to a development build.
 * The join check runs that version's bundled Client with this window's Session arguments and
 * environment; a version that joins starts through the window launcher at the place the asking
 * document shows, and this window then quits as Quit Orchbee does.
 */
function createWindowUpdate(sessionArguments: readonly string[]): DesktopWindowUpdate {
	const desktopRoot = windowDesktopRoot;
	const stem = distribution.artifactStem;
	const clientName = process.platform === 'win32' ? `${stem}.exe` : stem;
	const versionCohort = (root: string, version: string): string =>
		path.join(
			windowVersionResources({
				applicationName: distribution.applicationName,
				desktopRoot: root,
				platform: process.platform,
				version,
			}),
			PACKAGED_COHORT_DIRECTORY,
		);
	return new DesktopWindowUpdate({
		desktopRoot,
		fallbackFrom: readFallbackFromVersion(process.argv),
		onJoinable: async (version, place) => {
			if (desktopRoot === null) return false;
			const started = await relaunchWindow({
				desktopRoot,
				executablePath: process.execPath,
				place,
				platform: process.platform,
				processId: process.pid,
				quit: () => app.quit(),
				start: startDetached,
				versionLauncher:
					process.platform === 'darwin'
						? path.join(versionCohort(desktopRoot, version), `${stem}-desktop-launcher`)
						: null,
			});
			if (started) recordEvent('info', 'window-relaunch-started');
			else recordEvent('error', 'window-relaunch-failed');
			return started;
		},
		ownVersion,
		readPointer: readWindowPointer,
		recordEvent,
		restartForVersion: (root, version, request) =>
			restartForUpdate(
				path.join(versionCohort(root, version), clientName),
				sessionArguments,
				resolveClientEnvironment(),
				request,
			),
		restartSession: (request) =>
			restartForUpdate(
				resolveClientExecutable(),
				sessionArguments,
				resolveClientEnvironment(),
				request,
			),
		startVersionClient: (root, version) =>
			readServerStartAnswer(
				path.join(versionCohort(root, version), clientName),
				sessionArguments,
				resolveClientEnvironment(),
			),
		// A window started hidden has no background runtime and stops nothing.
		stopSession: () => background?.stopSession() ?? Promise.resolve(false),
		watchPointer: watchWindowPointer,
	});
}

/**
 * The Linux window icon: the package's menu icon, or in a checkout the brand's; null elsewhere.
 */
function resolveLinuxWindowIcon(): string | null {
	if (process.platform !== 'linux') return null;
	if (packaged || distribution.dataScope.kind !== 'development') {
		return path.join(process.resourcesPath, 'linux', 'icons', '256x256.png');
	}
	return path.join(
		distribution.dataScope.sourceRoot,
		'resources',
		'brand',
		'png',
		'icon',
		'orchbee-icon-256.png',
	);
}

/**
 * The icons of the notification area, the menu bar or the panel: the package's copy, or in a
 * checkout the generated brand assets, `.ico` files on Windows, template PNGs on macOS and PNGs on
 * Linux.
 */
function resolveTrayIconDirectory(): string {
	if (packaged || distribution.dataScope.kind !== 'development') {
		return path.join(process.resourcesPath, PACKAGED_TRAY_ICON_DIRECTORY);
	}
	const brand = path.join(distribution.dataScope.sourceRoot, 'resources', 'brand');
	if (process.platform === 'darwin') return path.join(brand, 'png', 'tray');
	if (process.platform === 'linux') return path.join(brand, 'png', 'linux', 'tray');
	return path.join(brand, 'ico', 'tray');
}

/**
 * Desktop's icon, from start until quit: in the notification area on Windows, the menu bar on macOS
 * and the panel on Linux. Closing the window while terminals run keeps Desktop running behind it,
 * and on macOS in the Dock whatever runs. A window started hidden for automation keeps no icon.
 */
async function createBackground(
	sessionArguments: readonly string[],
): Promise<DesktopBackgroundRuntime | undefined> {
	const platform = process.platform;
	if (windowHidden || (platform !== 'win32' && platform !== 'darwin' && platform !== 'linux'))
		return undefined;
	const record = new FileDesktopTrayRecord(dataRoot.trayStateFile);
	const clientExecutable = resolveClientExecutable();
	const clientEnvironment = resolveClientEnvironment();
	const applicationName = distribution.applicationName;
	const intl = createOrchbeeIntl(resolveOrchbeeLocale(app.getPreferredSystemLanguages()));
	return new DesktopBackgroundRuntime({
		applicationName,
		closeWindow: () => application?.window?.close(),
		createSurface: (actions) =>
			new DesktopBackgroundTray({
				applicationName,
				distribution: distribution.dataScope.kind,
				iconDirectory: resolveTrayIconDirectory(),
				intl,
				onNoticeShown: containListenerFault(recordEvent, 'tray', actions.onNoticeShown, undefined),
				onQuit: containListenerFault(recordEvent, 'tray', actions.onQuit, undefined),
				onQuitAll: containListenerFault(recordEvent, 'tray', actions.onQuitAll, undefined),
				onReopen: containListenerFault(recordEvent, 'tray', actions.onReopen, undefined),
				platform,
			}),
		intl,
		noticeShown: await record.readNoticeShown(),
		quit: () => app.quit(),
		readStopStatus: (stopId) =>
			readServerStopStatus(clientExecutable, sessionArguments, clientEnvironment, stopId),
		record,
		recordEvent,
		release: platform === 'darwin' ? 'close-window' : 'quit',
		reopen: () => {
			// A menu-bar menu leaves the application that was active before it, so Open Orchbee makes
			// Orchbee the active one; the window it shows would otherwise open behind.
			if (platform === 'darwin') app.focus({ steal: true });
			void application?.openWindow().catch(() => undefined);
		},
		requestSessionStop: (stopId) => application?.requestSessionStop(stopId) ?? false,
		showWindow: async () => {
			await application?.openWindow().catch(() => undefined);
			return application?.window;
		},
	});
}

async function resolveRendererUrl(): Promise<URL> {
	const developmentUrl = packaged ? undefined : process.env[RENDERER_URL_ENVIRONMENT_KEY];
	if (developmentUrl !== undefined) return new URL(developmentUrl);
	await installDesktopRendererScheme(
		protocol,
		packaged
			? path.join(process.resourcesPath, PACKAGED_RENDERER_DIRECTORY)
			: requiredEnvironment(RENDERER_ROOT_ENVIRONMENT_KEY),
		(fileUrl) => net.fetch(fileUrl),
	);
	return new URL(DESKTOP_RENDERER_URL);
}

/**
 * The menu, its window Actions, macOS Secure Keyboard Entry, and the diagnostic bundle with, on
 * Windows, its taskbar task; returns their removal.
 */
function installPlatformActions(application: DesktopServerClientApplication): () => void {
	const intl = createOrchbeeIntl(resolveOrchbeeLocale(app.getPreferredSystemLanguages()));
	const windowActions = new DesktopWindowActions(application);
	const diagnosticsActions = new DesktopDiagnosticsActions(
		new DesktopDiagnosticsRuntime({
			clientEnvironment: resolveClientEnvironment(),
			clientExecutable: resolveClientExecutable(),
			intl,
			recordEvent,
			window: () => application.window,
		}),
		process.platform,
	);
	saveDiagnosticBundle = () => diagnosticsActions.invokeSaveBundle('menu');
	// The title bar overlay hides the Windows menu bar, so the taskbar's Jump List holds the same
	// Action; a checkout's Electron has no identity of its own to hold one. The macOS menu bar always
	// shows, so its Help menu is enough.
	if (process.platform === 'win32' && packaged) {
		app.setUserTasks([
			{
				arguments: SAVE_DIAGNOSTIC_BUNDLE_SWITCH,
				description: intl.formatMessage(desktopDiagnosticsMessages.taskDescription),
				iconIndex: 0,
				iconPath: windowStartProgram,
				program: windowStartProgram,
				title: intl.formatMessage(desktopDiagnosticsMessages.saveBundle),
			},
		]);
	}
	const secureKeyboardEntry = new DesktopSecureKeyboardEntry(
		process.platform,
		{
			isEnabled: () => app.isSecureKeyboardEntryEnabled(),
			setEnabled: (enabled) => app.setSecureKeyboardEntryEnabled(enabled),
		},
		process.platform === 'darwin' && app.isActive(),
	);
	const secureKeyboardEntryActions = new DesktopSecureKeyboardEntryActions(secureKeyboardEntry);
	const activateSecureKeyboardEntry = (): void => secureKeyboardEntry.activate();
	const deactivateSecureKeyboardEntry = (): void => secureKeyboardEntry.deactivate();
	app.on('browser-window-focus', activateSecureKeyboardEntry);
	app.on('browser-window-blur', deactivateSecureKeyboardEntry);
	const menuBackground = background;
	const menuListener = <Args extends readonly unknown[], Result>(
		listener: (...args: Args) => Result,
		fallback: Result,
	): ((...args: Args) => Result) =>
		containListenerFault(recordEvent, 'application-menu', listener, fallback);
	const menu = installDesktopApplicationMenu(
		{
			background:
				menuBackground === undefined
					? undefined
					: {
							getQuitAllAvailability: () => ({
								kind: menuBackground.menuView.quitAll ? 'enabled' : 'disabled',
							}),
							getReopenAvailability: () => ({
								kind: menuBackground.menuView.windowShown ? 'hidden' : 'enabled',
							}),
							invokeQuitAll: menuListener(() => menuBackground.quitAllRequested(), undefined),
							invokeReopen: menuListener(
								() => void application.openWindow().catch(() => undefined),
								undefined,
							),
						},
			getSecureKeyboardEntryAvailability: () => secureKeyboardEntryActions.getToggleAvailability(),
			getToggleFullScreenAvailability: () => windowActions.getToggleFullScreenAvailability(),
			getToggleMaximizedAvailability: () => windowActions.getToggleMaximizedAvailability(),
			invokeToggleFullScreen: menuListener(
				(source) => windowActions.invokeToggleFullScreen(source),
				false,
			),
			invokeToggleMaximized: menuListener(
				(source) => windowActions.invokeToggleMaximized(source),
				false,
			),
			invokeToggleSecureKeyboardEntry: menuListener(
				(source) => secureKeyboardEntryActions.invokeToggle(source),
				false,
			),
			saveDiagnosticBundle: {
				getAvailability: () => diagnosticsActions.getSaveBundleAvailability(),
				invoke: menuListener((source) => diagnosticsActions.invokeSaveBundle(source), false),
			},
		},
		{ applicationName: distribution.applicationName, intl },
		process.platform,
	);
	return () => {
		app.off('browser-window-focus', activateSecureKeyboardEntry);
		app.off('browser-window-blur', deactivateSecureKeyboardEntry);
		menu.dispose();
		saveDiagnosticBundle = undefined;
		diagnosticsActions.dispose();
		secureKeyboardEntryActions.dispose();
		windowActions.dispose();
		secureKeyboardEntry.dispose();
	};
}

let application: DesktopServerClientApplication | undefined;
let background: DesktopBackgroundRuntime | undefined;
let removePlatformActions: (() => void) | undefined;
let windowUpdate: DesktopWindowUpdate | undefined;
/** The saved machines' process, started once Desktop runs. */
let machines: DesktopMachinesProcess | undefined;
let saveDiagnosticBundle: (() => boolean) | undefined;
let windowStoreTendTimer: ReturnType<typeof setTimeout> | undefined;
/** A Save Diagnostic Bundle task that started Orchbee waits for the Action to be installed. */
let diagnosticBundleRequested = process.argv.includes(SAVE_DIAGNOSTIC_BUNDLE_SWITCH);
let windowStateSettled = false;

/**
 * Schedules one background `--tend-window-store` run for a packaged window that uses a window
 * store, so retired window versions and flat-layout leftovers leave disk once any replaced process
 * has exited, without waiting for a future update.
 */
function scheduleWindowStoreTend(): void {
	if (windowDesktopRoot === null) return;
	const timer = setTimeout(() => {
		windowStoreTendTimer = undefined;
		const child = spawn(resolveClientExecutable(), ['--tend-window-store'], {
			cwd: app.getPath('home'),
			env: resolveClientEnvironment(),
			stdio: 'ignore',
			windowsHide: true,
		});
		child.on('error', () => undefined);
		child.unref();
	}, WINDOW_STORE_TEND_DELAY_MILLISECONDS);
	timer.unref();
	windowStoreTendTimer = timer;
}

/** Brings the window forward, so the save dialog opens over it, then starts the save. */
function requestDiagnosticBundle(): void {
	const save = saveDiagnosticBundle;
	if (save === undefined) {
		diagnosticBundleRequested = true;
		return;
	}
	void (application?.openWindow() ?? Promise.resolve()).then(save, save);
}

/**
 * Answers a second launch. The launch Setup makes when it finishes switches this window to the
 * version it installed, at the place the window shows; every other launch, or a switch that does
 * not start, only brings the window forward.
 */
async function answerSecondLaunch(): Promise<void> {
	const update = windowUpdate;
	const switched =
		update !== undefined &&
		(await isSetupLaunch(update)) &&
		(await update.switchIfOffered(() => update.askPlace()));
	if (!switched) await application?.openWindow().catch(() => undefined);
}

/** Whether this launch is the one Setup made: its marker names the version the store now offers. */
async function isSetupLaunch(update: DesktopWindowUpdate): Promise<boolean> {
	if (process.platform !== 'win32' || windowDesktopRoot === null) return false;
	await update.refresh();
	return takeSetupLaunch({
		desktopRoot: windowDesktopRoot,
		now: Date.now(),
		offeredVersion: update.offeredVersion,
	});
}

async function start(): Promise<void> {
	const rendererUrl = await resolveRendererUrl();
	if (!isPermittedDesktopRendererUrl(rendererUrl)) {
		throw new Error('The Desktop renderer URL is not permitted.');
	}
	const session =
		automationEnvironment[SESSION_ENVIRONMENT_KEY] ?? resolveDesktopSessionName(distribution);
	const sessionArguments = session === undefined ? [] : ['--session', session];
	background = await createBackground(sessionArguments);
	// A first instance has no Setup launch to answer; it deletes a marker that Setup left unread.
	if (process.platform === 'win32' && windowDesktopRoot !== null) {
		await discardSetupLaunch(windowDesktopRoot);
	}
	windowUpdate = createWindowUpdate(sessionArguments);
	windowUpdate.start();
	// The process answers whether this host reaches saved machines, as the TUI's capability does.
	machines = new DesktopMachinesProcess({
		clientEnvironment: resolveClientEnvironment(),
		clientExecutable: resolveClientExecutable(),
		recordEvent,
	});
	machines.start();
	const started = new DesktopServerClientApplication({
		applicationVersion: ownVersion,
		background,
		checkoutName: resolveDesktopCheckoutName(distribution, process.platform),
		clientEnvironment: resolveClientEnvironment(),
		clientExecutable: resolveClientExecutable(),
		distribution: distribution.dataScope.kind,
		exitsOnFirstStartAnswer: (refusal) => {
			if (
				!givesWayToReplacedVersion({
					announced: relaunchAnnounced,
					elapsedMilliseconds: performance.now() - launchedAt,
					refusal,
				})
			) {
				return false;
			}
			// The launcher that announced this window starts the version it replaced.
			recordEvent('warning', 'window-relaunch-gave-way');
			background?.dispose();
			windowUpdate?.dispose();
			app.exit(1);
			return true;
		},
		machines,
		platform: process.platform,
		preloadPath: path.join(__dirname, '..', 'preload', 'preload.cjs'),
		windowIcon: resolveLinuxWindowIcon(),
		recordEvent,
		relaunchPlace: readRelaunchPlace(process.argv),
		rendererUrl,
		sessionArguments,
		windowHidden,
		windowStateOwner: new DesktopWindowStateOwner(
			new FileDesktopWindowStateStore(dataRoot.windowStateFile, {
				upgradeBackup: createUpgradeBackup(dataRoot.path, ownVersion),
			}),
			process.platform,
			reportWindowStateRecord,
		),
		windowUpdate,
	});
	application = started;
	app.on('activate', () => void started.openWindow().catch(() => undefined));
	await started.openWindow();
	// The menu reads each Action's availability when it is installed and when it opens; a hidden
	// title bar never opens it, so it waits for the window its accelerators act on.
	removePlatformActions = installPlatformActions(started);
	if (diagnosticBundleRequested) {
		diagnosticBundleRequested = false;
		requestDiagnosticBundle();
	}
	scheduleWindowStoreTend();
}

if (app.requestSingleInstanceLock()) {
	// A second launch only brings this window forward, or opens one where macOS kept none; the one
	// exception is the launch Setup makes when it finishes, which switches this window to the version
	// it installed. The taskbar's task starts a diagnostic bundle save.
	app.on('second-instance', (_event, argv) => {
		if (argv.includes(SAVE_DIAGNOSTIC_BUNDLE_SWITCH)) {
			requestDiagnosticBundle();
			return;
		}
		void answerSecondLaunch();
	});
	recordEvent('info', 'started');
	void app
		.whenReady()
		.then(start)
		.catch((error: unknown) => {
			recordEvent('error', 'startup-failed');
			console.error('Orchbee Desktop startup failed.', error);
			process.exitCode = 1;
			windowStateSettled = true;
			app.quit();
		});
} else {
	windowStateSettled = true;
	app.quit();
}

app.on('before-quit', (event) => {
	// From here a close is never turned into a hide, whatever started the quit.
	background?.quitRequested();
	if (windowStateSettled || application === undefined) return;
	event.preventDefault();
	void application.settleWindowState().finally(() => {
		windowStateSettled = true;
		app.quit();
	});
});

app.on('window-all-closed', () => {
	if (process.platform !== 'darwin') app.quit();
});

// The development runtime restarts Main after a rebuild through an ordinary quit, which records the
// window's place first and closes the window even while terminals run; the Session keeps running, as
// it does after any quit.
if (!packaged && process.send !== undefined) {
	process.on('message', (message: unknown) => {
		if (DesktopDevelopmentRestartRequestSchema.safeParse(message).success) app.quit();
	});
}

app.once('will-quit', () => {
	if (windowStoreTendTimer !== undefined) {
		clearTimeout(windowStoreTendTimer);
		windowStoreTendTimer = undefined;
	}
	removePlatformActions?.();
	removePlatformActions = undefined;
	background?.dispose();
	windowUpdate?.dispose();
	// The process leaves the hub once its standard input closes, whether or not Main is still there.
	void machines?.stop();
	recordEvent('info', 'exited');
});

// Last, once every handler above is registered: from here a fault in one window, connection, message,
// or request is recorded and Main goes on.
containMainProcessFaults(recordEvent);
