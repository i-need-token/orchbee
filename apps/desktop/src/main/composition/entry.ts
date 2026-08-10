import path from 'node:path';

import { app, net, protocol } from 'electron';

import {
	DESKTOP_RENDERER_URL,
	installDesktopRendererScheme,
	registerDesktopRendererScheme,
} from '../desktop-renderer-protocol';
import { DesktopApplication } from './DesktopApplication';
import { resolveDesktopRuntimeConfiguration } from './resolveDesktopRuntimeConfiguration';

let desktopApplication: DesktopApplication | undefined;
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
				environment: process.env,
				homeDirectory: app.getPath('home'),
				...(app.isPackaged ? { packagedResourcesPath: process.resourcesPath } : {}),
				platform: process.platform,
				posixRuntimeBaseDirectory,
				userDataPath: app.getPath('userData'),
			});
			desktopApplication = new DesktopApplication({
				platform: process.platform,
				preloadPath: path.join(__dirname, '..', 'preload', 'preload.cjs'),
				rendererUrl,
				runtime,
				singleInstanceLockOwned,
				traceCopyPath: process.env['ORCHBEE_DESKTOP_TRACE'] === '1',
			});
			await desktopApplication.start();
			app.on('activate', () => void desktopApplication?.openWindow());
		})
		.catch((error: unknown) => {
			console.error('Orchbee Desktop startup failed.', error);
			bypassQuitGate = true;
			app.quit();
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
