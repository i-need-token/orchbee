import { enUSApplicationRoutingMessages } from './application-routing';
import { enUSDesktopApplicationLifetimeMessages } from './desktop-application-lifetime';
import { enUSDesktopBrowserAccessMessages } from './desktop-browser-access';
import { enUSDesktopWindowMessages } from './desktop-window';
import { enUSKeyboardShortcutMessages } from './keyboard-shortcuts';
import { enUSProjectedTerminalMessages } from './projected-terminal';
import { enUSTerminalSettingsMessages } from './terminal-settings';
import { enUSWorkbenchMessages } from './workbench';

export const enUSMessages = {
	...enUSApplicationRoutingMessages,
	...enUSDesktopApplicationLifetimeMessages,
	...enUSDesktopBrowserAccessMessages,
	...enUSDesktopWindowMessages,
	...enUSKeyboardShortcutMessages,
	...enUSProjectedTerminalMessages,
	...enUSTerminalSettingsMessages,
	...enUSWorkbenchMessages,
} as const;

export type OrchbeeMessageId = keyof typeof enUSMessages;
