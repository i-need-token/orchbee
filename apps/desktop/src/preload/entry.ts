import { ipcRenderer } from 'electron';

import { installDesktopApplicationLifetimeBridge } from './installDesktopApplicationLifetimeBridge';
import { installTerminalDocumentControlBridge } from './installTerminalDocumentControlBridge';
import { installTerminalProjectionControlBridge } from './installTerminalProjectionControlBridge';
import { installWorkbenchDocumentControlBridge } from './installWorkbenchDocumentControlBridge';

const security = {
	contextIsolated: process.contextIsolated,
	sandboxed: process.sandboxed,
};

installTerminalDocumentControlBridge(ipcRenderer, window, security);
installDesktopApplicationLifetimeBridge(ipcRenderer, window, security);
installTerminalProjectionControlBridge(ipcRenderer, window, security);
installWorkbenchDocumentControlBridge(ipcRenderer, window, security);
