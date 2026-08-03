import { ipcRenderer } from 'electron';

import { installDesktopClientSessionBridge } from './installDesktopClientSessionBridge';
import { installDesktopApplicationLifetimeBridge } from './installDesktopApplicationLifetimeBridge';
import { installTerminalProjectionControlBridge } from './installTerminalProjectionControlBridge';

const security = {
	contextIsolated: process.contextIsolated,
	sandboxed: process.sandboxed,
};

installDesktopClientSessionBridge(ipcRenderer, window, security);
installDesktopApplicationLifetimeBridge(ipcRenderer, window, security);
installTerminalProjectionControlBridge(ipcRenderer, window, security);
