import { ipcRenderer } from 'electron';

import { installProjectedAttachmentBridge } from './installProjectedAttachmentBridge';
import { installTerminalDocumentControlBridge } from './installTerminalDocumentControlBridge';

const security = {
	contextIsolated: process.contextIsolated,
	sandboxed: process.sandboxed,
};

installProjectedAttachmentBridge(ipcRenderer, window, security);
installTerminalDocumentControlBridge(ipcRenderer, window, security);
