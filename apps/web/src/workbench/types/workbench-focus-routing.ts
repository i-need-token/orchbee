import type * as React from 'react';

import type {
	WorkbenchFocusIntent,
	WorkbenchFocusResolution,
	WorkbenchTerminalFocusRequest,
} from '../model/resolveWorkbenchFocusTarget';

/** A pending request's claim on the keyboard. Any later press by the person ends it for good. */
export interface WorkbenchTerminalHandover {
	/** Gives the terminal the keyboard, once whatever surface asked has finished closing. */
	readonly complete: () => void;
	readonly held: () => boolean;
}

export interface WorkbenchFocusRouting {
	readonly beginTerminalHandover: () => WorkbenchTerminalHandover;
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
