import type {
	TerminalPresentationCommand,
	TerminalPresentationEvent,
	TerminalPresentationFailure,
	TerminalPresentationId,
	TerminalPresentationOperationId,
} from '@orchbee/workbench/terminal-presentation';
import type {
	WorkbenchSharedDocument,
	WorkbenchSharedPanelNode,
	WorkbenchSharedPresentation,
} from '@orchbee/workbench/document';

export interface DesktopTerminalIntentReconcilerDependencies {
	readonly createOperationId: () => TerminalPresentationOperationId;
	readonly execute: (command: TerminalPresentationCommand) => Promise<TerminalPresentationEvent>;
	readonly getTerminalExit: (
		presentationId: TerminalPresentationId,
	) => Extract<TerminalPresentationEvent, { readonly type: 'terminal-exited' }> | null;
}

interface DurableOperationRecord {
	readonly fingerprint: string;
	readonly result: TerminalPresentationEvent;
}

interface RetainRecord {
	readonly operationId: TerminalPresentationOperationId;
}

export class DesktopTerminalIntentReconciler {
	readonly #createOperationId: () => TerminalPresentationOperationId;
	#currentPresentationIds = new Set<TerminalPresentationId>();
	readonly #events = new Map<TerminalPresentationId, readonly TerminalPresentationEvent[]>();
	readonly #execute: DesktopTerminalIntentReconcilerDependencies['execute'];
	readonly #getTerminalExit: DesktopTerminalIntentReconcilerDependencies['getTerminalExit'];
	readonly #operations = new Map<TerminalPresentationOperationId, DurableOperationRecord>();
	readonly #retained = new Map<TerminalPresentationId, RetainRecord>();

	constructor(dependencies: DesktopTerminalIntentReconcilerDependencies) {
		this.#createOperationId = dependencies.createOperationId;
		this.#execute = dependencies.execute;
		this.#getTerminalExit = dependencies.getTerminalExit;
	}

	validateDocument(document: WorkbenchSharedDocument): boolean {
		const candidates = durableCommands(document);
		const local = new Map<TerminalPresentationOperationId, string>();
		for (const command of candidates) {
			const fingerprint = commandFingerprint(command);
			const localFingerprint = local.get(command.operationId);
			if (localFingerprint !== undefined && localFingerprint !== fingerprint) return false;
			local.set(command.operationId, fingerprint);
			const existing = this.#operations.get(command.operationId);
			if (existing !== undefined && existing.fingerprint !== fingerprint) return false;
		}
		return true;
	}

	getTerminalEvents(): readonly TerminalPresentationEvent[] {
		return [...this.#events.entries()]
			.sort(([left], [right]) => left.localeCompare(right))
			.flatMap(([, events]) => [...events]);
	}

	recordExternalEvent(
		event: Extract<TerminalPresentationEvent, { readonly type: 'terminal-exited' }>,
	): boolean {
		if (!this.#currentPresentationIds.has(event.presentationId)) return false;
		this.#recordEvent(event);
		return true;
	}

	async reconcileDocument(
		document: WorkbenchSharedDocument,
		publish: (event: TerminalPresentationEvent) => void,
	): Promise<void> {
		const presentations = sharedPresentations(document);
		const currentOperationIds = new Set<TerminalPresentationOperationId>(
			presentations.flatMap((presentation) =>
				'operationId' in presentation.intent ? [presentation.intent.operationId] : [],
			),
		);
		const currentPresentationIds = new Set<TerminalPresentationId>(
			presentations.map((presentation) => presentation.presentationId),
		);
		this.#currentPresentationIds = currentPresentationIds;
		for (const operationId of this.#operations.keys()) {
			if (!currentOperationIds.has(operationId)) this.#operations.delete(operationId);
		}
		for (const presentationId of this.#retained.keys()) {
			const presentation = presentations.find(
				(candidate) => candidate.presentationId === presentationId,
			);
			if (presentation?.intent.kind !== 'retain') this.#retained.delete(presentationId);
		}
		for (const presentationId of this.#events.keys()) {
			if (!currentPresentationIds.has(presentationId)) this.#events.delete(presentationId);
		}

		for (const presentation of presentations) {
			if (presentation.intent.kind === 'stopped') {
				this.#events.delete(presentation.presentationId);
				continue;
			}
			if (presentation.intent.kind === 'retain') {
				await this.#reconcileRetain(presentation, publish);
				continue;
			}
			const command = commandForPresentation(presentation);
			if (this.#operations.has(command.operationId)) continue;
			const result = await this.#executeCommand(command);
			this.#operations.set(command.operationId, {
				fingerprint: commandFingerprint(command),
				result,
			});
			this.#recordAndPublishResult(result, publish);
		}
	}

	async #reconcileRetain(
		presentation: WorkbenchSharedPresentation,
		publish: (event: TerminalPresentationEvent) => void,
	): Promise<void> {
		if (this.#retained.has(presentation.presentationId)) return;
		const operationId = this.#createOperationId();
		this.#retained.set(presentation.presentationId, { operationId });
		const previous = this.#events.get(presentation.presentationId) ?? [];
		const previousExit = previous.find(
			(event): event is Extract<TerminalPresentationEvent, { readonly type: 'terminal-exited' }> =>
				event.type === 'terminal-exited',
		);
		const alreadyRunning = previous.some(
			(event) =>
				event.type === 'terminal-provision-completed' ||
				event.type === 'terminal-restore-completed',
		);
		const started = {
			operationId,
			presentationId: presentation.presentationId,
			type: 'terminal-restore-started',
		} as const;
		this.#recordEvent(started);
		if (alreadyRunning) {
			const completed = {
				operationId,
				presentationId: presentation.presentationId,
				type: 'terminal-restore-completed',
			} as const;
			this.#recordEvent(completed);
			if (previousExit !== undefined) this.#recordEvent(previousExit);
			return;
		}
		publish(started);
		const result = await this.#executeCommand({
			launchCwd: presentation.launchCwd,
			operationId,
			presentationId: presentation.presentationId,
			type: 'terminal-restore-requested',
		});
		this.#recordAndPublishResult(result, publish);
	}

	async #executeCommand(command: TerminalPresentationCommand): Promise<TerminalPresentationEvent> {
		try {
			const result = await this.#execute(command);
			return resultMatchesCommand(command, result)
				? result
				: failedResult(command, mismatchFailure);
		} catch {
			return failedResult(command, unavailableFailure);
		}
	}

	#recordEvent(event: TerminalPresentationEvent): void {
		const current = this.#events.get(event.presentationId) ?? [];
		let next: readonly TerminalPresentationEvent[];
		switch (event.type) {
			case 'terminal-restore-started':
				next = [event];
				break;
			case 'terminal-restore-completed':
			case 'terminal-restore-failed':
				next = [
					...current.filter(
						(candidate) =>
							candidate.type === 'terminal-restore-started' &&
							candidate.operationId === event.operationId,
					),
					event,
				];
				break;
			case 'terminal-exited':
				next = [...current.filter((candidate) => candidate.type !== 'terminal-exited'), event];
				break;
			default:
				next = [event];
		}
		this.#events.set(event.presentationId, next);
	}

	#recordAndPublishResult(
		result: TerminalPresentationEvent,
		publish: (event: TerminalPresentationEvent) => void,
	): void {
		this.#recordEvent(result);
		publish(result);
		if (
			result.type !== 'terminal-provision-completed' &&
			result.type !== 'terminal-restore-completed'
		) {
			return;
		}
		const exit = this.#getTerminalExit(result.presentationId);
		if (exit === null) return;
		this.#recordEvent(exit);
		publish(exit);
	}
}

const mismatchFailure: TerminalPresentationFailure = {
	code: 'terminal-effect-result-mismatch',
	retryable: false,
};
const unavailableFailure: TerminalPresentationFailure = {
	code: 'terminal-effect-unavailable',
	retryable: true,
};

function durableCommands(
	document: WorkbenchSharedDocument,
): readonly TerminalPresentationCommand[] {
	return sharedPresentations(document)
		.filter(
			(presentation) =>
				presentation.intent.kind !== 'retain' && presentation.intent.kind !== 'stopped',
		)
		.map(commandForPresentation);
}

function sharedPresentations(
	document: WorkbenchSharedDocument,
): readonly WorkbenchSharedPresentation[] {
	return document.terminalContexts.flatMap((context) =>
		context.panelWorkspace.tabs.flatMap((tab) => presentationsInNode(tab.root)),
	);
}

function presentationsInNode(
	node: WorkbenchSharedPanelNode,
): readonly WorkbenchSharedPresentation[] {
	return node.kind === 'panel'
		? [node.primaryPresentation, ...node.childPresentations]
		: node.children.flatMap((child) => presentationsInNode(child.node));
}

function commandForPresentation(
	presentation: WorkbenchSharedPresentation,
): TerminalPresentationCommand {
	switch (presentation.intent.kind) {
		case 'provision':
			return {
				launchCwd: presentation.launchCwd,
				operationId: presentation.intent.operationId,
				presentationId: presentation.presentationId,
				type: 'terminal-provision-requested',
			};
		case 'close':
		case 'stop':
			return {
				operationId: presentation.intent.operationId,
				presentationId: presentation.presentationId,
				type: 'terminal-close-requested',
			};
		case 'retain':
		case 'stopped':
			throw new Error('A terminal without a durable operation cannot produce a durable command.');
	}
}

function commandFingerprint(command: TerminalPresentationCommand): string {
	return JSON.stringify(command);
}

function resultMatchesCommand(
	command: TerminalPresentationCommand,
	result: TerminalPresentationEvent,
): boolean {
	if (result.type === 'terminal-exited' || result.type === 'terminal-restore-started') return false;
	if (
		result.operationId !== command.operationId ||
		result.presentationId !== command.presentationId
	) {
		return false;
	}
	switch (command.type) {
		case 'terminal-provision-requested':
			return (
				result.type === 'terminal-provision-completed' ||
				result.type === 'terminal-provision-failed'
			);
		case 'terminal-restore-requested':
			return (
				result.type === 'terminal-restore-completed' || result.type === 'terminal-restore-failed'
			);
		case 'terminal-close-requested':
			return result.type === 'terminal-close-completed' || result.type === 'terminal-close-failed';
	}
}

function failedResult(
	command: TerminalPresentationCommand,
	failure: TerminalPresentationFailure,
): TerminalPresentationEvent {
	switch (command.type) {
		case 'terminal-provision-requested':
			return {
				failure,
				operationId: command.operationId,
				presentationId: command.presentationId,
				type: 'terminal-provision-failed',
			};
		case 'terminal-restore-requested':
			return {
				failure,
				operationId: command.operationId,
				presentationId: command.presentationId,
				type: 'terminal-restore-failed',
			};
		case 'terminal-close-requested':
			return {
				failure,
				operationId: command.operationId,
				presentationId: command.presentationId,
				type: 'terminal-close-failed',
			};
	}
}
