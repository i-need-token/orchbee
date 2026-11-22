import type { TerminalPresentationId } from '@orchbee/workbench-client-protocol';

/** Whether Orchbee answers an Agent's routine confirmations: never, for this run, or for its type. */
export type WorkbenchAgentConfirmationChoice = 'agent-type' | 'off' | 'run';

/** What a person answers about an Agent's confirmations; `no` answers an offer and ends offers. */
export type WorkbenchAgentConfirmationAnswer = WorkbenchAgentConfirmationChoice | 'no';

/** Why the confirmation an Agent shows now waits for the person. */
export type WorkbenchAgentConfirmationWait =
	| 'answered'
	| 'changing'
	| 'home-workspace'
	| 'host-too-old'
	| 'leaves-workspace'
	| 'marked-dangerous'
	| 'not-answered'
	| 'not-understood'
	| 'not-visible'
	| 'person'
	| 'protected-file'
	| 'repeating';

/** The last confirmation Orchbee answered for an Agent run. */
export interface WorkbenchAgentConfirmationPress {
	readonly kind: 'command' | 'create' | 'edit';
	/** Grows by one with each answer, so the same command answered twice is said twice. */
	readonly serial: number;
	/** The command, or the file's path. */
	readonly text: string;
}

/** How the Server answers the routine confirmations of the Agent one Presentation's terminal runs. */
export interface WorkbenchAgentConfirmation {
	/** The Agent's reported label, as its observation names it. */
	readonly agent: string;
	readonly choice: WorkbenchAgentConfirmationChoice;
	/** The Server offers to turn answering on. */
	readonly offer: boolean;
	readonly pressed: WorkbenchAgentConfirmationPress | null;
	/** Answering stopped for this run because the same confirmation kept coming back. */
	readonly stopped: boolean;
	readonly wait: WorkbenchAgentConfirmationWait | null;
}

/**
 * The Server's automatic answers to Agent confirmations. A Presentation is listed only while its
 * terminal runs an Agent the Server can answer for.
 */
export interface WorkbenchAgentConfirmations {
	/** Sets the choice for the Agent in one Presentation. Resolves false when it was not applied. */
	choose(
		presentationId: TerminalPresentationId,
		choice: WorkbenchAgentConfirmationChoice,
	): Promise<boolean>;
	/** Answers an offer with no, which ends offers for every Agent. Resolves false when not applied. */
	decline(presentationId: TerminalPresentationId): Promise<boolean>;
	getSnapshot(): ReadonlyMap<TerminalPresentationId, WorkbenchAgentConfirmation>;
	subscribe(listener: () => void): () => void;
}
