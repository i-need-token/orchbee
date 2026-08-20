import type {
	ActionDisposable,
	ActionExecution,
	ActionRuntime,
	ActionScope,
	ActionTarget,
	PreparedActionInvocation,
} from '@orchbee/actions';
import type { ComponentType } from 'react';

import type { TerminalPresentation } from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchTerminalBellEffect } from '../model/WorkbenchTerminalBellEffectCoordinator';

export type WorkbenchTerminalPresentation = TerminalPresentation;

export interface WorkbenchTerminalActionContextCaptureRequest {
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
	readonly actionRuntime: ActionRuntime;
	readonly actionScope: ActionScope;
	readonly admitBellEffect: (event: WorkbenchTerminalBellEffect) => boolean;
	readonly bindActionContextProvider: (
		provider: WorkbenchTerminalActionContextProvider,
	) => ActionDisposable;
	readonly presentation: WorkbenchTerminalPresentation;
}

export type WorkbenchTerminalSurface = ComponentType<WorkbenchTerminalSurfaceProps>;
