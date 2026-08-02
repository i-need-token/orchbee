import { ipcRenderer } from 'electron';

import { installTerminalDocumentControlBridge } from './installTerminalDocumentControlBridge';
import { installTerminalProjectionControlBridge } from './installTerminalProjectionControlBridge';

const security = {
	contextIsolated: process.contextIsolated,
	sandboxed: process.sandboxed,
};

installTerminalDocumentControlBridge(ipcRenderer, window, security);
installTerminalProjectionControlBridge(ipcRenderer, window, security);
