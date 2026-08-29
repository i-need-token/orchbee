import type * as React from 'react';

import type {
	WorkbenchFocusIntent,
	WorkbenchFocusResolution,
	WorkbenchTerminalFocusRequest,
} from '../model/resolveWorkbenchFocusTarget';

export interface WorkbenchFocusRouting {
	readonly focus: (intent: WorkbenchFocusIntent) => WorkbenchFocusResolution;
	readonly getResolution: (intent: WorkbenchFocusIntent) => WorkbenchFocusResolution;
	readonly onBlurCapture: (event: React.FocusEvent<HTMLDivElement>) => void;
	readonly onFocusCapture: (event: React.FocusEvent<HTMLDivElement>) => void;
	readonly sidebarFocusRequest: number;
	readonly tabStripFocusRequest: number;
	readonly terminalAutoFocusAllowed: boolean;
	readonly terminalFocusRequest: WorkbenchTerminalFocusRequest | null;
	readonly requestTerminalFocus: () => boolean;
}
