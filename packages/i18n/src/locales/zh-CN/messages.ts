import { zhCNWebsiteShellMessages } from './website-shell';
import { zhCNWebsiteHomeMessages } from './website-home';
import { zhCNWebsiteTerminalMessages } from './website-terminal';
import { zhCNWebsiteDesktopMessages } from './website-desktop';
import { zhCNWebsiteCliMessages } from './website-cli';
import { zhCNWebsiteProductMessages } from './website-product';
import type { OrchbeeMessageId } from '../en-US';
import { zhCNAgentObservationMessages } from './agent-observation';
import { zhCNAgentStartMessages } from './agent-start';
import { zhCNApplicationRoutingMessages } from './application-routing';

import { zhCNDesktopDiagnosticsMessages } from './desktop-diagnostics';
import { zhCNDesktopMenuMessages } from './desktop-menu';
import { zhCNDesktopSecureKeyboardEntryMessages } from './desktop-secure-keyboard-entry';
import { zhCNDesktopTrayMessages } from './desktop-tray';

import { zhCNDesktopWindowMessages } from './desktop-window';
import { zhCNKeyboardShortcutMessages } from './keyboard-shortcuts';
import { zhCNProjectedTerminalMessages } from './projected-terminal';
import { zhCNSessionConnectionMessages } from './session-connection';

import { zhCNTerminalSettingsMessages } from './terminal-settings';
import { zhCNWorkbenchMessages } from './workbench';

export const zhCNMessages = {
	...zhCNWebsiteShellMessages,
	...zhCNWebsiteHomeMessages,
	...zhCNWebsiteTerminalMessages,
	...zhCNWebsiteDesktopMessages,
	...zhCNWebsiteCliMessages,
	...zhCNWebsiteProductMessages,
	...zhCNAgentObservationMessages,
	...zhCNAgentStartMessages,
	...zhCNApplicationRoutingMessages,

	...zhCNDesktopDiagnosticsMessages,
	...zhCNDesktopMenuMessages,
	...zhCNDesktopSecureKeyboardEntryMessages,
	...zhCNDesktopTrayMessages,

	...zhCNDesktopWindowMessages,
	...zhCNKeyboardShortcutMessages,
	...zhCNProjectedTerminalMessages,
	...zhCNSessionConnectionMessages,

	...zhCNTerminalSettingsMessages,
	...zhCNWorkbenchMessages,
} as const satisfies Readonly<Record<OrchbeeMessageId, string>>;
