import { ipcRenderer, webUtils } from 'electron';

import { installDesktopFileDropBridge } from './installDesktopFileDropBridge';
import { installDesktopServerConnectionBridge } from './installDesktopServerConnectionBridge';
import { installDesktopWindowCaptionBridge } from './installDesktopWindowCaptionBridge';
import { installDesktopWindowFaultBridge } from './installDesktopWindowFaultBridge';

installDesktopServerConnectionBridge(ipcRenderer, window, {
	contextIsolated: process.contextIsolated,
	sandboxed: process.sandboxed,
});
installDesktopWindowCaptionBridge(ipcRenderer, window);
installDesktopWindowFaultBridge(ipcRenderer, window);
installDesktopFileDropBridge(window, (file) => webUtils.getPathForFile(file), process.platform);
