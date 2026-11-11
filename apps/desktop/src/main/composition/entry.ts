/**
 * The Desktop application as a Server Client: one window whose document holds one Client connection
 * through an `orchbee server connect` relay.
 */
import path from 'node:path';

import {
	createOrchbeeIntl,
	desktopDiagnosticsMessages,
	desktopSecureKeyboardEntryMessages,
	desktopWindowMessages,
	resolveOrchbeeLocale,
} from '@orchbee/i18n';
import { app, dialog, net, protocol } from 'electron';

import { DesktopDevelopmentRestartRequestSchema } from '../../development';
import { resolveDesktopCheckoutName, resolveDesktopSessionName } from '../../distribution';
import { installDesktopApplicationMenu } from '../application-menu';
import { DesktopBackgroundRuntime, FileDesktopTrayRecord } from '../background-tray';
import {
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
import { readServerStopStatus } from '../server-connection';
import { createUpgradeBackup } from '../upgrade-backup';
import { DesktopWindowActions } from '../window-actions';
import {
	DesktopWindowStateOwner,
	type DesktopWindowStateRecordReporter,
	FileDesktopWindowStateStore,
} from '../window-state';
import { DesktopDataRootRefusal } from './data-root-version-record';
import { DesktopServerClientApplication } from './DesktopServerClientApplication';
import { isPackagedDesktop } from './isPackagedDesktop';
import { type DesktopDataRoot, prepareDesktopDataRoot } from './prepareDesktopDataRoot';
import { resolveDesktopApplicationDistribution } from './resolveDesktopApplicationDistribution';

const CLIENT_EXECUTABLE_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_CLIENT_EXECUTABLE';
const RENDERER_ROOT_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_RENDERER_ROOT';
const RENDERER_URL_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_RENDERER_URL';
const SESSION_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_SESSION';
const WINDOW_HIDDEN_ENVIRONMENT_KEY = 'ORCHBEE_DESKTOP_WINDOW_HIDDEN';
/** What the Windows taskbar's Save Diagnostic Bundle task starts Orchbee with. */
const SAVE_DIAGNOSTIC_BUNDLE_SWITCH = '--save-diagnostic-bundle';
// Cross-runtime contract: search for `packaged-desktop-resources` before changing these values.
const PACKAGED_COHORT_DIRECTORY = 'cohort';
const PACKAGED_RENDERER_DIRECTORY = 'dist';
const PACKAGED_TRAY_ICON_DIRECTORY = 'tray';

const packaged = isPackagedDesktop(process);
registerDesktopRendererScheme(protocol);
const distribution = resolveDesktopApplicationDistribution({
	applicationPath: app.getAppPath(),
	environment: process.env,
	packaged,
	platform: process.platform,
});
app.setName(distribution.applicationName);
const dataRoot = prepareOrRefuseDesktopDataRoot();
app.setPath('userData', dataRoot.electronDirectory);
// The natives write the same daily logs on Windows only until ORC-328, and so does Main.
const recordEvent: DesktopDiagnosticRecorder =
	process.platform === 'win32'
		? new DesktopDiagnosticLog(dataRoot.logDirectory).recorder()
		: () => undefined;

/**
 * Prepares the data root, or, when its layout is one this version cannot open, explains why and
 * exits before anything reads or writes it.
 */
function prepareOrRefuseDesktopDataRoot(): DesktopDataRoot {
	try {
		return prepareDesktopDataRoot({
			applicationVersion: app.getVersion(),
			distribution,
			homeDirectory: app.getPath('home'),
			homeOverride: process.env['ORCHBEE_HOME'],
			platform: process.platform,
		});
	} catch (error: unknown) {
		if (!(error instanceof DesktopDataRootRefusal)) throw error;
		const intl = createOrchbeeIntl(resolveOrchbeeLocale(app.getPreferredSystemLanguages()));
		let message: string;
		if (error.layout === undefined) {
			message = intl.formatMessage(desktopWindowMessages.dataFolderUnreadable, {
				record: error.recordPath,
				root: error.root,
			});
		} else if (error.newestVersion === undefined) {
			message = intl.formatMessage(desktopWindowMessages.dataFolderLayoutNewerUnnamed, {
				root: error.root,
			});
		} else {
			message = intl.formatMessage(desktopWindowMessages.dataFolderLayoutNewer, {
				root: error.root,
				version: error.newestVersion,
			});
		}
		dialog.showErrorBox(intl.formatMessage(desktopWindowMessages.dataFolderRefused), message);
		app.exit(1);
		throw error;
	}
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

/** The notification-area icons: the package's copy, or in a checkout the generated brand assets. */
function resolveTrayIconDirectory(): string {
	if (packaged || distribution.dataScope.kind !== 'development') {
		return path.join(process.resourcesPath, PACKAGED_TRAY_ICON_DIRECTORY);
	}
	return path.join(distribution.dataScope.sourceRoot, 'resources', 'brand', 'ico', 'tray');
}

/**
 * On Windows, closing the window while terminals run keeps Desktop in the notification area; a
 * window started hidden for automation keeps none. macOS and Linux keep their close behavior.
 */
async function createBackground(
	sessionArguments: readonly string[],
): Promise<DesktopBackgroundRuntime | undefined> {
	if (process.platform !== 'win32' || process.env[WINDOW_HIDDEN_ENVIRONMENT_KEY] !== undefined) {
		return undefined;
	}
	const record = new FileDesktopTrayRecord(dataRoot.trayStateFile);
	const clientExecutable = resolveClientExecutable();
	const clientEnvironment = resolveClientEnvironment();
	return new DesktopBackgroundRuntime({
		applicationName: distribution.applicationName,
		distribution: distribution.dataScope.kind,
		iconDirectory: resolveTrayIconDirectory(),
		intl: createOrchbeeIntl(resolveOrchbeeLocale(app.getPreferredSystemLanguages())),
		noticeShown: await record.readNoticeShown(),
		quit: () => app.quit(),
		readStopStatus: (stopId) =>
			readServerStopStatus(clientExecutable, sessionArguments, clientEnvironment, stopId),
		record,
		reopen: () => void application?.openWindow().catch(() => undefined),
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
 * The menu, its window Actions, macOS Secure Keyboard Entry, and the Windows diagnostic bundle with
 * its taskbar task; returns their removal.
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
	// Action; a checkout's Electron has no identity of its own to hold one.
	if (process.platform === 'win32' && packaged) {
		app.setUserTasks([
			{
				arguments: SAVE_DIAGNOSTIC_BUNDLE_SWITCH,
				description: intl.formatMessage(desktopDiagnosticsMessages.taskDescription),
				iconIndex: 0,
				iconPath: process.execPath,
				program: process.execPath,
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
	const menu = installDesktopApplicationMenu(
		{
			getSecureKeyboardEntryAvailability: () => secureKeyboardEntryActions.getToggleAvailability(),
			getToggleFullScreenAvailability: () => windowActions.getToggleFullScreenAvailability(),
			getToggleMaximizedAvailability: () => windowActions.getToggleMaximizedAvailability(),
			invokeToggleFullScreen: (source) => windowActions.invokeToggleFullScreen(source),
			invokeToggleMaximized: (source) => windowActions.invokeToggleMaximized(source),
			invokeToggleSecureKeyboardEntry: (source) => secureKeyboardEntryActions.invokeToggle(source),
			saveDiagnosticBundle: {
				categoryLabel: intl.formatMessage(desktopDiagnosticsMessages.category),
				getAvailability: () => diagnosticsActions.getSaveBundleAvailability(),
				invoke: (source) => diagnosticsActions.invokeSaveBundle(source),
				label: intl.formatMessage(desktopDiagnosticsMessages.saveBundle),
			},
		},
		{
			secureKeyboardEntry: intl.formatMessage(desktopSecureKeyboardEntryMessages.toggle),
			securityCategory: intl.formatMessage(desktopSecureKeyboardEntryMessages.category),
			toggleFullScreen: intl.formatMessage(desktopWindowMessages.toggleFullScreen),
			toggleMaximized: intl.formatMessage(desktopWindowMessages.toggleMaximized),
			windowCategory: intl.formatMessage(desktopWindowMessages.category),
		},
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
let saveDiagnosticBundle: (() => boolean) | undefined;
/** A Save Diagnostic Bundle task that started Orchbee waits for the Action to be installed. */
let diagnosticBundleRequested = process.argv.includes(SAVE_DIAGNOSTIC_BUNDLE_SWITCH);
let windowStateSettled = false;

/** Brings the window forward, so the save dialog opens over it, then starts the save. */
function requestDiagnosticBundle(): void {
	const save = saveDiagnosticBundle;
	if (save === undefined) {
		diagnosticBundleRequested = true;
		return;
	}
	void (application?.openWindow() ?? Promise.resolve()).then(save, save);
}

async function start(): Promise<void> {
	const rendererUrl = await resolveRendererUrl();
	if (!isPermittedDesktopRendererUrl(rendererUrl)) {
		throw new Error('The Desktop renderer URL is not permitted.');
	}
	const session = process.env[SESSION_ENVIRONMENT_KEY] ?? resolveDesktopSessionName(distribution);
	const sessionArguments = session === undefined ? [] : ['--session', session];
	background = await createBackground(sessionArguments);
	const started = new DesktopServerClientApplication({
		background,
		checkoutName: resolveDesktopCheckoutName(distribution, process.platform),
		clientEnvironment: resolveClientEnvironment(),
		clientExecutable: resolveClientExecutable(),
		distribution: distribution.dataScope.kind,
		platform: process.platform,
		preloadPath: path.join(__dirname, '..', 'preload', 'preload.cjs'),
		recordEvent,
		rendererUrl,
		sessionArguments,
		windowHidden: process.env[WINDOW_HIDDEN_ENVIRONMENT_KEY] !== undefined,
		windowStateOwner: new DesktopWindowStateOwner(
			new FileDesktopWindowStateStore(dataRoot.windowStateFile, {
				upgradeBackup: createUpgradeBackup(dataRoot.path, app.getVersion()),
			}),
			process.platform,
			reportWindowStateRecord,
		),
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
}

if (app.requestSingleInstanceLock()) {
	// Starting Orchbee again brings this window forward, or opens one where macOS kept none; the
	// taskbar's task also starts a diagnostic bundle save.
	app.on('second-instance', (_event, argv) => {
		if (argv.includes(SAVE_DIAGNOSTIC_BUNDLE_SWITCH)) requestDiagnosticBundle();
		else void application?.openWindow().catch(() => undefined);
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
	removePlatformActions?.();
	removePlatformActions = undefined;
	background?.dispose();
	recordEvent('info', 'exited');
});
