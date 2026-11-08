import { enUSWebsiteMessages } from './website';
import { enUSApplicationRoutingMessages } from './application-routing';
import { enUSAgentIntegrationMessages } from './agent-integrations';
import { enUSAgentObservationMessages } from './agent-observation';

import { enUSDesktopDiagnosticsMessages } from './desktop-diagnostics';
import { enUSDesktopSecureKeyboardEntryMessages } from './desktop-secure-keyboard-entry';

import { enUSDesktopWindowMessages } from './desktop-window';
import { enUSKeyboardShortcutMessages } from './keyboard-shortcuts';
import { enUSProjectedTerminalMessages } from './projected-terminal';
import { enUSSessionConnectionMessages } from './session-connection';

import { enUSTerminalSettingsMessages } from './terminal-settings';
import { enUSTerminalSettingsCatalogMessages } from './terminal-settings-catalog';
import { enUSWorkbenchMessages } from './workbench';

export const enUSMessages = {
	...enUSWebsiteMessages,
	...enUSApplicationRoutingMessages,
	...enUSAgentIntegrationMessages,
	...enUSAgentObservationMessages,

	...enUSDesktopDiagnosticsMessages,
	...enUSDesktopSecureKeyboardEntryMessages,

	...enUSDesktopWindowMessages,
	...enUSKeyboardShortcutMessages,
	...enUSProjectedTerminalMessages,
	...enUSSessionConnectionMessages,

	...enUSTerminalSettingsMessages,
	...enUSTerminalSettingsCatalogMessages,
	...enUSWorkbenchMessages,
} as const;

export type OrchbeeMessageId = keyof typeof enUSMessages;
