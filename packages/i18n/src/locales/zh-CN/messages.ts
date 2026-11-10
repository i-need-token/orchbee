import { zhCNWebsiteMessages } from './website';
import type { OrchbeeMessageId } from '../en-US';
import { zhCNAgentIntegrationMessages } from './agent-integrations';
import { zhCNAgentObservationMessages } from './agent-observation';
import { zhCNApplicationRoutingMessages } from './application-routing';

import { zhCNDesktopDiagnosticsMessages } from './desktop-diagnostics';
import { zhCNDesktopSecureKeyboardEntryMessages } from './desktop-secure-keyboard-entry';
import { zhCNDesktopTrayMessages } from './desktop-tray';

import { zhCNDesktopWindowMessages } from './desktop-window';
import { zhCNKeyboardShortcutMessages } from './keyboard-shortcuts';
import { zhCNProjectedTerminalMessages } from './projected-terminal';
import { zhCNSessionConnectionMessages } from './session-connection';

import { zhCNTerminalSettingsMessages } from './terminal-settings';
import { zhCNTerminalSettingsCatalogMessages } from './terminal-settings-catalog';
import { zhCNWorkbenchMessages } from './workbench';

export const zhCNMessages = {
	...zhCNWebsiteMessages,
	...zhCNAgentIntegrationMessages,
	...zhCNAgentObservationMessages,
	...zhCNApplicationRoutingMessages,

	...zhCNDesktopDiagnosticsMessages,
	...zhCNDesktopSecureKeyboardEntryMessages,
	...zhCNDesktopTrayMessages,

	...zhCNDesktopWindowMessages,
	...zhCNKeyboardShortcutMessages,
	...zhCNProjectedTerminalMessages,
	...zhCNSessionConnectionMessages,

	...zhCNTerminalSettingsMessages,
	...zhCNTerminalSettingsCatalogMessages,
	...zhCNWorkbenchMessages,
} as const satisfies Readonly<Record<OrchbeeMessageId, string>>;
