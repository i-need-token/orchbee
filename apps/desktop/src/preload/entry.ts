import { ipcRenderer } from 'electron';

import { installDesktopServerConnectionBridge } from './installDesktopServerConnectionBridge';
import { installDesktopWindowCaptionBridge } from './installDesktopWindowCaptionBridge';
import { installDesktopWindowFaultBridge } from './installDesktopWindowFaultBridge';

installDesktopServerConnectionBridge(ipcRenderer, window, {
	contextIsolated: process.contextIsolated,
	sandboxed: process.sandboxed,
});
installDesktopWindowCaptionBridge(ipcRenderer, window);
installDesktopWindowFaultBridge(ipcRenderer, window);
