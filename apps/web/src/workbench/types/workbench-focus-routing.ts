import type * as React from 'react';

import type {
	WorkbenchFocusIntent,
	WorkbenchFocusRegion,
	WorkbenchFocusResolution,
	WorkbenchTerminalFocusRequest,
} from '../model/resolveWorkbenchFocusTarget';

/** A region that a part of the Workbench shows only at times, and that says how it takes the keys. */
export type WorkbenchSuppliedFocusRegion = Extract<
	WorkbenchFocusRegion,
	'file-bar' | 'file-overlay'
>;

/** A pending request's claim on the keyboard. Any later press by the person ends it for good. */
export interface WorkbenchTerminalHandover {
	/** Gives the terminal the keyboard, once whatever surface asked has finished closing. */
	readonly complete: () => void;
	readonly held: () => boolean;
}

/**
 * Who asks for a focus move. A command surface's row never counts that surface as blocking: it
 * resolves from the region the closing surface gives focus back to and moves focus after that.
 */
export interface WorkbenchFocusInvocation {
	readonly fromCommandSurface: boolean;
}

export interface WorkbenchFocusRouting {
	readonly beginTerminalHandover: () => WorkbenchTerminalHandover;
	readonly focus: (
		intent: WorkbenchFocusIntent,
		invocation?: WorkbenchFocusInvocation,
	) => WorkbenchFocusResolution;
	readonly getResolution: (
		intent: WorkbenchFocusIntent,
		invocation?: WorkbenchFocusInvocation,
	) => WorkbenchFocusResolution;
	readonly onBlurCapture: (event: React.FocusEvent<HTMLDivElement>) => void;
	readonly onFocusCapture: (event: React.FocusEvent<HTMLDivElement>) => void;
	readonly sidebarFocusRequest: number;
	/**
	 * Adds a region to the cycle and the direct focus routes, with how it takes the keys, until the
	 * returned function withdraws it.
	 */
	readonly supplyRegion: (region: WorkbenchSuppliedFocusRegion, focus: () => void) => () => void;
	readonly tabStripFocusRequest: number;
	readonly terminalAutoFocusAllowed: boolean;
	readonly terminalFocusRequest: WorkbenchTerminalFocusRequest | null;
	readonly requestTerminalFocus: () => boolean;
}
