import type { OrchbeeMessageId } from '../en-US';
import { zhCNAgentIntegrationMessages } from './agent-integrations';
import { zhCNAgentObservationMessages } from './agent-observation';
import { zhCNApplicationRoutingMessages } from './application-routing';
import { zhCNBrowserAuthenticationMessages } from './browser-authentication';
import { zhCNDesktopApplicationLifetimeMessages } from './desktop-application-lifetime';
import { zhCNDesktopBrowserAccessMessages } from './desktop-browser-access';
import { zhCNDesktopSecureKeyboardEntryMessages } from './desktop-secure-keyboard-entry';
import { zhCNDesktopSshProjectMessages } from './desktop-ssh-project';
import { zhCNDesktopWindowMessages } from './desktop-window';
import { zhCNKeyboardShortcutMessages } from './keyboard-shortcuts';
import { zhCNProjectedTerminalMessages } from './projected-terminal';
import { zhCNRemoteDirectoryBrowserMessages } from './remote-directory-browser';
import { zhCNSshConnectionManagementMessages } from './ssh-connection-management';
import { zhCNTerminalSettingsMessages } from './terminal-settings';
import { zhCNTerminalSettingsCatalogMessages } from './terminal-settings-catalog';
import { zhCNWorkbenchMessages } from './workbench';

export const zhCNMessages = {
	...zhCNAgentIntegrationMessages,
	...zhCNAgentObservationMessages,
	...zhCNApplicationRoutingMessages,
	...zhCNBrowserAuthenticationMessages,
	...zhCNDesktopApplicationLifetimeMessages,
	...zhCNDesktopBrowserAccessMessages,
	...zhCNDesktopSecureKeyboardEntryMessages,
	...zhCNDesktopSshProjectMessages,
	...zhCNDesktopWindowMessages,
	...zhCNKeyboardShortcutMessages,
	...zhCNProjectedTerminalMessages,
	...zhCNRemoteDirectoryBrowserMessages,
	...zhCNSshConnectionManagementMessages,
	...zhCNTerminalSettingsMessages,
	...zhCNTerminalSettingsCatalogMessages,
	...zhCNWorkbenchMessages,
} as const satisfies Readonly<Record<OrchbeeMessageId, string>>;
