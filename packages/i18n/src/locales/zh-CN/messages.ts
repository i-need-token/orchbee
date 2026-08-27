import type { OrchbeeMessageId } from '../en-US';
import { zhCNApplicationRoutingMessages } from './application-routing';
import { zhCNBrowserAuthenticationMessages } from './browser-authentication';
import { zhCNDesktopApplicationLifetimeMessages } from './desktop-application-lifetime';
import { zhCNDesktopBrowserAccessMessages } from './desktop-browser-access';
import { zhCNDesktopSecureKeyboardEntryMessages } from './desktop-secure-keyboard-entry';
import { zhCNDesktopWindowMessages } from './desktop-window';
import { zhCNKeyboardShortcutMessages } from './keyboard-shortcuts';
import { zhCNProjectedTerminalMessages } from './projected-terminal';
import { zhCNTerminalSettingsMessages } from './terminal-settings';
import { zhCNTerminalSettingsCatalogMessages } from './terminal-settings-catalog';
import { zhCNWorkbenchMessages } from './workbench';

export const zhCNMessages = {
	...zhCNApplicationRoutingMessages,
	...zhCNBrowserAuthenticationMessages,
	...zhCNDesktopApplicationLifetimeMessages,
	...zhCNDesktopBrowserAccessMessages,
	...zhCNDesktopSecureKeyboardEntryMessages,
	...zhCNDesktopWindowMessages,
	...zhCNKeyboardShortcutMessages,
	...zhCNProjectedTerminalMessages,
	...zhCNTerminalSettingsMessages,
	...zhCNTerminalSettingsCatalogMessages,
	...zhCNWorkbenchMessages,
} as const satisfies Readonly<Record<OrchbeeMessageId, string>>;
