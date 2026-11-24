import { ipcRenderer, webUtils } from 'electron';

import { installDesktopFileDropBridge } from './installDesktopFileDropBridge';
import { installDesktopMachineSessionBridge } from './installDesktopMachineSessionBridge';
import { installDesktopServerConnectionBridge } from './installDesktopServerConnectionBridge';
import { installDesktopWindowCaptionBridge } from './installDesktopWindowCaptionBridge';
import { installDesktopWindowFaultBridge } from './installDesktopWindowFaultBridge';

const connectedDocument = installDesktopServerConnectionBridge(ipcRenderer, window, {
	contextIsolated: process.contextIsolated,
	sandboxed: process.sandboxed,
});
installDesktopMachineSessionBridge(ipcRenderer, window, connectedDocument);
installDesktopWindowCaptionBridge(ipcRenderer, window);
installDesktopWindowFaultBridge(ipcRenderer, window);
installDesktopFileDropBridge(window, (file) => webUtils.getPathForFile(file), process.platform);
