import type { OrchbeeMessageId } from '../en-US';
import { zhCNApplicationRoutingMessages } from './application-routing';
import { zhCNDesktopApplicationLifetimeMessages } from './desktop-application-lifetime';
import { zhCNDesktopBrowserAccessMessages } from './desktop-browser-access';
import { zhCNDesktopWindowMessages } from './desktop-window';
import { zhCNKeyboardShortcutMessages } from './keyboard-shortcuts';
import { zhCNProjectedTerminalMessages } from './projected-terminal';
import { zhCNTerminalSettingsMessages } from './terminal-settings';
import { zhCNWorkbenchMessages } from './workbench';

export const zhCNMessages = {
	...zhCNApplicationRoutingMessages,
	...zhCNDesktopApplicationLifetimeMessages,
	...zhCNDesktopBrowserAccessMessages,
	...zhCNDesktopWindowMessages,
	...zhCNKeyboardShortcutMessages,
	...zhCNProjectedTerminalMessages,
	...zhCNTerminalSettingsMessages,
	...zhCNWorkbenchMessages,
} as const satisfies Readonly<Record<OrchbeeMessageId, string>>;
