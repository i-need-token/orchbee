import path from 'node:path';
import { hostname } from 'node:os';

import { createOrchbeeIntl, desktopWindowMessages, resolveOrchbeeLocale } from '@orchbee/i18n';
import { app, net, protocol } from 'electron';

import {
	DESKTOP_DEVELOPMENT_RESTART_PLAN_ENVIRONMENT_KEY,
	DesktopDevelopmentRestartPlanStore,
	DesktopDevelopmentRestartRequestSchema,
} from '../../development';
import {
	DESKTOP_RENDERER_URL,
	installDesktopRendererScheme,
	registerDesktopRendererScheme,
} from '../desktop-renderer-protocol';
import { DesktopWindowActions, installDesktopWindowActionMenu } from '../window-actions';
import { DesktopApplication } from './DesktopApplication';
import { resolveDesktopRuntimeConfiguration } from './resolveDesktopRuntimeConfiguration';

let desktopApplication: DesktopApplication | undefined;
let desktopWindowActionMenu: ReturnType<typeof installDesktopWindowActionMenu> | undefined;
let desktopWindowActions: DesktopWindowActions | undefined;
let developmentRestartPlanStore: DesktopDevelopmentRestartPlanStore | undefined;
let bypassQuitGate = false;
registerDesktopRendererScheme(protocol);
const singleInstanceLockOwned = app.requestSingleInstanceLock();

if (!singleInstanceLockOwned) {
	bypassQuitGate = true;
	app.quit();
} else {
	app.on('second-instance', () => {
		if (desktopApplication?.hasWindow) {
			desktopApplication.focusWindow();
		} else {
			void desktopApplication?.openWindow().catch(() => undefined);
		}
	});

	void app
		.whenReady()
		.then(async () => {
			let rendererUrl: string;
			if (app.isPackaged) {
				await installDesktopRendererScheme(
					protocol,
					path.join(process.resourcesPath, 'dist'),
					(fileUrl) => net.fetch(fileUrl),
				);
				rendererUrl = DESKTOP_RENDERER_URL;
			} else {
				const developmentRendererUrl = process.env['ORCHBEE_DESKTOP_RENDERER_URL'];
				if (developmentRendererUrl === undefined) {
					throw new Error('ORCHBEE_DESKTOP_RENDERER_URL is required in development.');
				}
				rendererUrl = developmentRendererUrl;
			}
			let posixRuntimeBaseDirectory: string | undefined;
			if (process.platform === 'linux') {
				const getEffectiveUserId = process.geteuid;
				if (getEffectiveUserId === undefined) {
					throw new Error('Linux effective-user identity is unavailable.');
				}
				posixRuntimeBaseDirectory = `/run/user/${getEffectiveUserId()}`;
			} else if (process.platform !== 'win32') {
				posixRuntimeBaseDirectory = app.getPath('temp');
			}
			const runtime = resolveDesktopRuntimeConfiguration({
				applicationPath: app.getAppPath(),
				...(app.isPackaged
					? {}
					: {
							developmentShellIntegrationDirectory: path.resolve(
								__dirname,
								'..',
								'..',
								'resources',
								'shell-integration',
							),
						}),
				environment: process.env,
				homeDirectory: app.getPath('home'),
				...(app.isPackaged ? { packagedResourcesPath: process.resourcesPath } : {}),
				platform: process.platform,
				posixRuntimeBaseDirectory,
				terminalHostName: hostname(),
				userDataPath: app.getPath('userData'),
			});
			if (!app.isPackaged) {
				const restartPlanPath = process.env[DESKTOP_DEVELOPMENT_RESTART_PLAN_ENVIRONMENT_KEY];
				if (restartPlanPath !== undefined) {
					developmentRestartPlanStore = new DesktopDevelopmentRestartPlanStore(restartPlanPath);
				}
			}
			desktopApplication = new DesktopApplication({
				...(developmentRestartPlanStore === undefined
					? {}
					: {
							prepareDevelopmentHostRestart: (documentRevision) =>
								developmentRestartPlanStore?.markReady(documentRevision) ?? Promise.resolve(false),
						}),
				platform: process.platform,
				preloadPath: path.join(__dirname, '..', 'preload', 'preload.cjs'),
				rendererUrl,
				runtime,
				singleInstanceLockOwned,
				traceCopyPath: process.env['ORCHBEE_DESKTOP_TRACE'] === '1',
			});
			await desktopApplication.start();
			const intl = createOrchbeeIntl(resolveOrchbeeLocale(app.getPreferredSystemLanguages()));
			desktopWindowActions = new DesktopWindowActions(desktopApplication);
			desktopWindowActionMenu = installDesktopWindowActionMenu(
				desktopWindowActions,
				{
					category: intl.formatMessage(desktopWindowMessages.category),
					toggleFullScreen: intl.formatMessage(desktopWindowMessages.toggleFullScreen),
					toggleMaximized: intl.formatMessage(desktopWindowMessages.toggleMaximized),
				},
				process.platform,
			);
			app.on('activate', () => void desktopApplication?.openWindow());
		})
		.catch((error: unknown) => {
			console.error('Orchbee Desktop startup failed.', error);
			bypassQuitGate = true;
			app.quit();
		});
}

if (!app.isPackaged) {
	process.on('message', (message: unknown) => {
		const parsed = DesktopDevelopmentRestartRequestSchema.safeParse(message);
		if (!parsed.success || desktopApplication === undefined) return;
		if (parsed.data.reason === 'terminal-host' && developmentRestartPlanStore === undefined) {
			return;
		}
		console.info(
			`Orchbee Desktop received development restart request ${parsed.data.generation} (${parsed.data.reason}).`,
		);
		void desktopApplication
			.requestExitAndQuit(
				parsed.data.reason === 'terminal-host' ? 'development-host-restart' : 'update-restart',
			)
			.then(() => {
				if (desktopApplication?.quitPrepared !== true) {
					console.warn('Orchbee Desktop development restart did not reach a ready exit.');
				}
			})
			.catch((error: unknown) => {
				console.error('Orchbee Desktop development restart failed.', error);
			});
	});
}

app.on('before-quit', (event) => {
	if (bypassQuitGate || desktopApplication === undefined || desktopApplication.quitPrepared) {
		return;
	}
	event.preventDefault();
	void desktopApplication?.requestExitAndQuit('user-quit');
});

app.on('window-all-closed', () => {
	if (process.platform !== 'darwin') app.quit();
});

app.once('will-quit', () => {
	desktopWindowActionMenu?.dispose();
	desktopWindowActionMenu = undefined;
	desktopWindowActions?.dispose();
	desktopWindowActions = undefined;
});
