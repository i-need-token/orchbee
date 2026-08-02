import { ipcRenderer } from 'electron';

import { installTerminalDocumentControlBridge } from './installTerminalDocumentControlBridge';
import { installTerminalProjectionControlBridge } from './installTerminalProjectionControlBridge';
import { installWorkbenchTopologyControlBridge } from './installWorkbenchTopologyControlBridge';

const security = {
	contextIsolated: process.contextIsolated,
	sandboxed: process.sandboxed,
};

installTerminalDocumentControlBridge(ipcRenderer, window, security);
installTerminalProjectionControlBridge(ipcRenderer, window, security);
installWorkbenchTopologyControlBridge(ipcRenderer, window, security);
