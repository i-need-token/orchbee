import { enUSApplicationRoutingMessages } from './application-routing';
import { enUSBrowserAuthenticationMessages } from './browser-authentication';
import { enUSDesktopApplicationLifetimeMessages } from './desktop-application-lifetime';
import { enUSDesktopBrowserAccessMessages } from './desktop-browser-access';
import { enUSDesktopSecureKeyboardEntryMessages } from './desktop-secure-keyboard-entry';
import { enUSDesktopSshProjectMessages } from './desktop-ssh-project';
import { enUSDesktopWindowMessages } from './desktop-window';
import { enUSKeyboardShortcutMessages } from './keyboard-shortcuts';
import { enUSProjectedTerminalMessages } from './projected-terminal';
import { enUSTerminalSettingsMessages } from './terminal-settings';
import { enUSTerminalSettingsCatalogMessages } from './terminal-settings-catalog';
import { enUSWorkbenchMessages } from './workbench';

export const enUSMessages = {
	...enUSApplicationRoutingMessages,
	...enUSBrowserAuthenticationMessages,
	...enUSDesktopApplicationLifetimeMessages,
	...enUSDesktopBrowserAccessMessages,
	...enUSDesktopSecureKeyboardEntryMessages,
	...enUSDesktopSshProjectMessages,
	...enUSDesktopWindowMessages,
	...enUSKeyboardShortcutMessages,
	...enUSProjectedTerminalMessages,
	...enUSTerminalSettingsMessages,
	...enUSTerminalSettingsCatalogMessages,
	...enUSWorkbenchMessages,
} as const;

export type OrchbeeMessageId = keyof typeof enUSMessages;
