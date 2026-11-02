import { ipcRenderer } from 'electron';

import { installDesktopServerConnectionBridge } from './installDesktopServerConnectionBridge';

installDesktopServerConnectionBridge(ipcRenderer, window, {
	contextIsolated: process.contextIsolated,
	sandboxed: process.sandboxed,
});
