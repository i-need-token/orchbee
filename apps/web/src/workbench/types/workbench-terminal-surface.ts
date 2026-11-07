import type {
	ActionDisposable,
	ActionExecution,
	ActionInvocationSource,
	ActionRuntime,
	ActionScope,
	ActionTarget,
	PreparedActionInvocation,
} from '@orchbee/actions';
import type { ComponentType } from 'react';

import type { TerminalPresentation } from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchTerminalBellEffect } from '../model/WorkbenchTerminalBellEffectCoordinator';

export type WorkbenchTerminalPresentation = TerminalPresentation;
export type WorkbenchTerminalPresentationMode = 'active' | 'incoming' | 'retained';
export type WorkbenchTerminalPresentationReadiness = 'activation-ready' | 'display-ready';

interface WorkbenchTerminalActionContextCaptureRequest {
	readonly altKey: boolean;
	readonly clientX: number;
	readonly clientY: number;
	readonly ctrlKey: boolean;
	readonly metaKey: boolean;
	readonly shiftKey: boolean;
}

export type WorkbenchTerminalActionContextCapture =
	| { readonly kind: 'terminal-owned' }
	| {
			readonly actionInputs?: Readonly<Record<string, unknown>> | undefined;
			readonly focus?: Readonly<Record<string, string | boolean | null>> | undefined;
			readonly kind: 'product';
			readonly target?: ActionTarget | undefined;
	  };

export interface WorkbenchTerminalActionContextProvider {
	capture(
		request: WorkbenchTerminalActionContextCaptureRequest,
	): WorkbenchTerminalActionContextCapture;
	executePrepared(invocation: PreparedActionInvocation): ActionExecution;
}

export interface WorkbenchTerminalSurfaceProps {
	/** Accepted terminal editing input, distinct from focus, navigation and input rejection. */
	readonly onTerminalEdit?: (() => void) | undefined;
	readonly actionRuntime: ActionRuntime;
	readonly actionScope: ActionScope;
	readonly admitBellEffect: (event: WorkbenchTerminalBellEffect) => boolean;
	readonly autoFocus: boolean;
	readonly focusRequest: number;
	readonly bindActionContextProvider: (
		provider: WorkbenchTerminalActionContextProvider,
	) => ActionDisposable;
	/**
	 * Closes this surface's Panel as the Panel's own Close does, which closes the Tab when the Panel
	 * is its last. A surface that cannot show a terminal offers it.
	 */
	readonly closeTerminal: (source: ActionInvocationSource) => void;
	/** Opens a new Tab as the Workbench's New Tab does. */
	readonly createTab: (source: ActionInvocationSource) => void;
	/**
	 * Starts this surface's terminal after a restore held it for an unavailable directory: in the
	 * saved directory again, or in the home folder. Absent where nothing can hold a restore.
	 */
	readonly restoreTerminal?: ((directory: 'home' | 'saved') => Promise<void>) | undefined;
	/** Report actual display readiness for both initial active and incoming surfaces. */
	readonly onPresentationReady: (readiness: WorkbenchTerminalPresentationReadiness) => void;
	readonly presentation: WorkbenchTerminalPresentation;
	readonly presentationMode: WorkbenchTerminalPresentationMode;
}

export type WorkbenchTerminalSurface = ComponentType<WorkbenchTerminalSurfaceProps>;
