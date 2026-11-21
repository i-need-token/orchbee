import { enUSWebsiteShellMessages } from './website-shell';
import { enUSWebsiteHomeMessages } from './website-home';
import { enUSWebsiteTerminalMessages } from './website-terminal';
import { enUSWebsiteDesktopMessages } from './website-desktop';
import { enUSWebsiteCliMessages } from './website-cli';
import { enUSWebsiteProductMessages } from './website-product';
import { enUSApplicationRoutingMessages } from './application-routing';
import { enUSAgentObservationMessages } from './agent-observation';
import { enUSAgentStartMessages } from './agent-start';

import { enUSDesktopDiagnosticsMessages } from './desktop-diagnostics';
import { enUSDesktopMenuMessages } from './desktop-menu';
import { enUSDesktopSecureKeyboardEntryMessages } from './desktop-secure-keyboard-entry';
import { enUSDesktopTrayMessages } from './desktop-tray';

import { enUSDesktopWindowMessages } from './desktop-window';
import { enUSKeyboardShortcutMessages } from './keyboard-shortcuts';
import { enUSProjectedTerminalMessages } from './projected-terminal';
import { enUSSessionConnectionMessages } from './session-connection';

import { enUSTerminalSettingsMessages } from './terminal-settings';
import { enUSWorkbenchMessages } from './workbench';

export const enUSMessages = {
	...enUSWebsiteShellMessages,
	...enUSWebsiteHomeMessages,
	...enUSWebsiteTerminalMessages,
	...enUSWebsiteDesktopMessages,
	...enUSWebsiteCliMessages,
	...enUSWebsiteProductMessages,
	...enUSApplicationRoutingMessages,
	...enUSAgentObservationMessages,
	...enUSAgentStartMessages,

	...enUSDesktopDiagnosticsMessages,
	...enUSDesktopMenuMessages,
	...enUSDesktopSecureKeyboardEntryMessages,
	...enUSDesktopTrayMessages,

	...enUSDesktopWindowMessages,
	...enUSKeyboardShortcutMessages,
	...enUSProjectedTerminalMessages,
	...enUSSessionConnectionMessages,

	...enUSTerminalSettingsMessages,
	...enUSWorkbenchMessages,
} as const;

export type OrchbeeMessageId = keyof typeof enUSMessages;
