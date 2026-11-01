import { contextBridge, ipcRenderer, webUtils } from 'electron';

import { installDesktopRendererBootstrapBridge } from './installDesktopRendererBootstrapBridge';
import { installDesktopRendererFilePathBridge } from './installDesktopRendererFilePathBridge';
import { installDesktopServerConnectionBridge } from './installDesktopServerConnectionBridge';

const security = {
	contextIsolated: process.contextIsolated,
	sandboxed: process.sandboxed,
};

installDesktopRendererBootstrapBridge(ipcRenderer, window, security);
installDesktopServerConnectionBridge(ipcRenderer, window, security);
installDesktopRendererFilePathBridge(contextBridge, webUtils, process.platform);
