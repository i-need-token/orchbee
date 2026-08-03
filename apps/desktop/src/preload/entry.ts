import { ipcRenderer } from 'electron';

import { installDesktopRendererBootstrapBridge } from './installDesktopRendererBootstrapBridge';

const security = {
	contextIsolated: process.contextIsolated,
	sandboxed: process.sandboxed,
};

installDesktopRendererBootstrapBridge(ipcRenderer, window, security);
