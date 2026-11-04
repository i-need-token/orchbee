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

interface WorkbenchTerminalBroadcastKeyModifiers {
	readonly alt: boolean;
	readonly altLeft: boolean;
	readonly altRight: boolean;
	readonly altGraph: boolean;
	readonly capsLock: boolean;
	readonly control: boolean;
	readonly controlRight: boolean;
	readonly meta: boolean;
	readonly metaRight: boolean;
	readonly numLock: boolean;
	readonly shift: boolean;
	readonly shiftRight: boolean;
}

export type WorkbenchTerminalBroadcastInput =
	| {
			readonly kind: 'key';
			readonly code: string;
			readonly key: string;
			readonly modifiers: WorkbenchTerminalBroadcastKeyModifiers;
			readonly repeat: boolean;
	  }
	| { readonly kind: 'text'; readonly text: string };

export type WorkbenchTerminalBroadcastInputReadiness =
	| { readonly kind: 'ready'; readonly revision: string }
	| { readonly kind: 'unavailable' };

export type WorkbenchTerminalBroadcastInputResult =
	| { readonly kind: 'blocked' }
	| { readonly kind: 'failed' }
	| { readonly kind: 'sent' };

/** A transient capability owned by one mounted terminal surface and its current attachment. */
export interface WorkbenchTerminalBroadcastInputSink {
	getReadiness(): WorkbenchTerminalBroadcastInputReadiness;
	send(
		input: WorkbenchTerminalBroadcastInput,
		expectedRevision: string,
	): Promise<WorkbenchTerminalBroadcastInputResult>;
	subscribe(listener: () => void): ActionDisposable;
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
	readonly bindBroadcastInputSink: (sink: WorkbenchTerminalBroadcastInputSink) => ActionDisposable;
	/**
	 * Closes this surface's Panel as the Panel's own Close does, which closes the Tab when the Panel
	 * is its last. A surface that cannot show a terminal offers it.
	 */
	readonly closeTerminal: (source: ActionInvocationSource) => void;
	/** Opens a new Tab as the Workbench's New Tab does. */
	readonly createTab: (source: ActionInvocationSource) => void;
	/** Report actual display readiness for both initial active and incoming surfaces. */
	readonly onPresentationReady: (readiness: WorkbenchTerminalPresentationReadiness) => void;
	readonly presentation: WorkbenchTerminalPresentation;
	readonly presentationMode: WorkbenchTerminalPresentationMode;
}

export type WorkbenchTerminalSurface = ComponentType<WorkbenchTerminalSurfaceProps>;
