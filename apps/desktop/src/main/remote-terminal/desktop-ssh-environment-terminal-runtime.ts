import type {
	TerminalExited,
	TerminalFailed,
	TerminalGeometry,
	TerminalIdentity,
} from '@orchbee/terminal-protocol';
import type {
	TerminalPresentationCommand,
	TerminalPresentationEvent,
	TerminalPresentationId,
} from '@orchbee/workbench/terminal-presentation';

import type { DesktopTerminalExitRecordResult } from '../terminal-document-control';
import type {
	DesktopTerminalProjectionLease,
	DesktopTerminalProjectionOpenOptions,
} from '../terminal-projection-control';

type DesktopSshEnvironmentTerminalOperationResult = Exclude<
	TerminalPresentationEvent,
	| { readonly type: 'terminal-exited' }
	| { readonly type: 'terminal-failed' }
	| { readonly type: 'terminal-restore-started' }
>;

export interface DesktopSshEnvironmentTerminalRuntimeHandle {
	commitExitPublication?(event: TerminalExited): boolean;
	dispose(): Promise<void>;
	execute(
		command: TerminalPresentationCommand,
	): Promise<DesktopSshEnvironmentTerminalOperationResult>;
	findPresentationByIdentity(identity: TerminalIdentity): TerminalPresentationId | null;
	getDurableTerminalExit(presentationId: TerminalPresentationId): TerminalExited | null;
	getTerminalFailure(presentationId: TerminalPresentationId): TerminalFailed | null;
	initialize(): Promise<
		| { readonly kind: 'ready' }
		| { readonly kind: 'conflict' }
		| {
				readonly kind: 'unavailable';
				readonly reason: 'invalid' | 'read-failed' | 'write-failed';
		  }
	>;
	open(
		presentationId: TerminalPresentationId,
		geometry: TerminalGeometry,
		options?: DesktopTerminalProjectionOpenOptions,
	): Promise<DesktopTerminalProjectionLease>;
	recordExitOutcome(event: TerminalExited): Promise<DesktopTerminalExitRecordResult>;
	recordFailure(event: TerminalFailed): Promise<TerminalPresentationId | null>;
	releasePendingExit?(event: TerminalExited): void;
	setIdentityObservedListener?(listener: (identity: TerminalIdentity) => void): void;
}
