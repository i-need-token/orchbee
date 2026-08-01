import { ipcRenderer } from 'electron';

import { installProjectedAttachmentBridge } from './installProjectedAttachmentBridge';

installProjectedAttachmentBridge(ipcRenderer, window, {
	contextIsolated: process.contextIsolated,
	sandboxed: process.sandboxed,
});
