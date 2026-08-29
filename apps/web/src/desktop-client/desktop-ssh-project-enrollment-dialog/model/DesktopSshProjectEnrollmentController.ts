import {
	DesktopSshProjectEnrollmentInputSchema,
	DesktopSshProjectDirectorySelectionResultSchema,
	type DesktopSshEnvironmentAddResult,
	type DesktopSshEnvironmentConnectionResult,
	type DesktopSshEnvironmentId,
	type DesktopSshProjectConnectionResult,
	type DesktopSshProjectDirectorySelectionResult,
	type DesktopSshProjectDirectorySnapshot,
	type DesktopSshProjectEnrollmentFailureReason,
	type DesktopSshProjectEnrollmentProgressStage,
	type DesktopSshProjectEnrollmentResult,
	type DesktopSshProjectFolderAttachmentFailureReason,
	type DesktopSshProjectFolderAttachmentResult,
	type DesktopSshProjectInteractionResult,
} from '@orchbee/desktop-client-protocol';
import type {
	WorkbenchEnvironmentProjectConnectionCapability,
	WorkbenchEnvironmentProjectConnectionStatus,
} from '@orchbee/workbench';

import type {
	DesktopSshConnectionManagementControl,
	DesktopSshConnectionManagementFailure,
	DesktopSshConnectionManagementSnapshot,
} from '../../../ssh-connection-management';
import type {
	DesktopSshProjectEnrollmentClient,
	DesktopSshProjectInteractionPrompt,
} from './DesktopSshProjectEnrollmentClient';

export interface DesktopSshProjectEnrollmentForm {
	readonly destination: string;
}

export type DesktopSshProjectEnrollmentFailureCategory =
	| 'busy'
	| 'connection'
	| 'control'
	| 'project-attachment'
	| 'project-path'
	| 'storage'
	| 'target'
	| 'unsupported'
	| 'workbench';

interface DesktopSshProjectEnrollmentDialogBase {
	readonly canNavigate: boolean;
	readonly directory: DesktopSshProjectDirectorySnapshot | null;
	readonly failure: DesktopSshProjectEnrollmentFailureCategory | null;
	readonly pathInput: string;
	readonly phase: 'directory' | 'editing' | 'failed' | 'prompt' | 'working';
	readonly progress: DesktopSshProjectEnrollmentProgressStage | null;
	readonly prompt: DesktopSshProjectInteractionPrompt | null;
}

export type DesktopSshProjectEnrollmentDialogState =
	| { readonly kind: 'closed' }
	| (DesktopSshProjectEnrollmentDialogBase & {
			readonly canSubmit: boolean;
			readonly form: DesktopSshProjectEnrollmentForm;
			readonly kind: 'enrollment';
	  })
	| (DesktopSshProjectEnrollmentDialogBase & {
			readonly canSubmit: boolean;
			readonly form: DesktopSshProjectEnrollmentForm;
			readonly kind: 'attachment';
			readonly projectId: string;
	  })
	| (DesktopSshProjectEnrollmentDialogBase & {
			readonly kind: 'connection';
			readonly projectId: string;
	  })
	| (DesktopSshProjectEnrollmentDialogBase & {
			readonly environmentId: DesktopSshEnvironmentId;
			readonly environmentName: string;
			readonly kind: 'environment-connection';
	  });

export interface DesktopSshProjectEnrollmentControllerOptions {
	readonly client: DesktopSshProjectEnrollmentControllerClient;
	readonly onEnrollmentCommitted: (
		result: Extract<DesktopSshProjectEnrollmentResult, { readonly kind: 'created' | 'existing' }>,
	) => Promise<boolean>;
	readonly onFolderAttachmentCommitted: (
		result: Extract<
			DesktopSshProjectFolderAttachmentResult,
			{ readonly kind: 'attached' | 'existing' }
		>,
	) => Promise<boolean>;
}

export interface DesktopSshProjectEnrollmentControllerClient {
	attachFolder: DesktopSshProjectEnrollmentClient['attachFolder'];
	connect: DesktopSshProjectEnrollmentClient['connect'];
	disconnect: DesktopSshProjectEnrollmentClient['disconnect'];
	addEnvironment: DesktopSshProjectEnrollmentClient['addEnvironment'];
	connectEnvironment: DesktopSshProjectEnrollmentClient['connectEnvironment'];
	disconnectEnvironment: DesktopSshProjectEnrollmentClient['disconnectEnvironment'];
	dispose: DesktopSshProjectEnrollmentClient['dispose'];
	enroll: DesktopSshProjectEnrollmentClient['enroll'];
	getStatuses: DesktopSshProjectEnrollmentClient['getStatuses'];
	getEnvironmentCatalog: DesktopSshProjectEnrollmentClient['getEnvironmentCatalog'];
	refreshEnvironmentCatalog: DesktopSshProjectEnrollmentClient['refreshEnvironmentCatalog'];
	subscribeEnvironmentCatalog: DesktopSshProjectEnrollmentClient['subscribeEnvironmentCatalog'];
	subscribeStatuses: DesktopSshProjectEnrollmentClient['subscribeStatuses'];
}

export interface DesktopSshProjectEnrollmentDialogController {
	close(): void;
	getDialogSnapshot(): DesktopSshProjectEnrollmentDialogState;
	navigateDirectory(path: string): void;
	openFolderAttachment(projectId: string): void;
	resolvePrompt(result: DesktopSshProjectInteractionResult): void;
	retry(): void;
	selectDirectory(): void;
	submit(): void;
	subscribeDialog(listener: Listener): () => void;
	updateDirectoryPath(path: string): void;
	updateForm(update: Partial<DesktopSshProjectEnrollmentForm>): void;
}

type Listener = () => void;

const EMPTY_FORM: DesktopSshProjectEnrollmentForm = {
	destination: '',
};

export class DesktopSshProjectEnrollmentController
	implements WorkbenchEnvironmentProjectConnectionCapability, DesktopSshConnectionManagementControl
{
	#activeAbortController: AbortController | undefined;
	readonly #client: DesktopSshProjectEnrollmentControllerClient;
	#connectionStatuses: readonly WorkbenchEnvironmentProjectConnectionStatus[];
	#dialogState: DesktopSshProjectEnrollmentDialogState = { kind: 'closed' };
	readonly #dialogListeners = new Set<Listener>();
	#disposed = false;
	readonly #managementListeners = new Set<Listener>();
	#managementSnapshot: DesktopSshConnectionManagementSnapshot;
	readonly #onEnrollmentCommitted: DesktopSshProjectEnrollmentControllerOptions['onEnrollmentCommitted'];
	readonly #onFolderAttachmentCommitted: DesktopSshProjectEnrollmentControllerOptions['onFolderAttachmentCommitted'];
	#operationGeneration = 0;
	#promptDeferred:
		| ReturnType<typeof createDeferred<DesktopSshProjectInteractionResult>>
		| undefined;
	#directoryDeferred:
		| ReturnType<typeof createDeferred<DesktopSshProjectDirectorySelectionResult>>
		| undefined;
	readonly #statusListeners = new Set<Listener>();
	readonly #unsubscribeStatuses: () => void;
	readonly #unsubscribeEnvironmentCatalog: () => void;

	constructor(options: DesktopSshProjectEnrollmentControllerOptions) {
		this.#client = options.client;
		this.#onEnrollmentCommitted = options.onEnrollmentCommitted;
		this.#onFolderAttachmentCommitted = options.onFolderAttachmentCommitted;
		this.#connectionStatuses = mapConnectionStatuses(options.client.getStatuses());
		this.#managementSnapshot = {
			catalog: options.client.getEnvironmentCatalog(),
			failure: null,
			pending: null,
		};
		this.#unsubscribeStatuses = options.client.subscribeStatuses(() => {
			this.#connectionStatuses = mapConnectionStatuses(this.#client.getStatuses());
			for (const listener of this.#statusListeners) listener();
		});
		this.#unsubscribeEnvironmentCatalog = options.client.subscribeEnvironmentCatalog(() => {
			this.#setManagement({
				...this.#managementSnapshot,
				catalog: this.#client.getEnvironmentCatalog(),
			});
		});
	}

	readonly getManagementSnapshot = (): DesktopSshConnectionManagementSnapshot =>
		this.#managementSnapshot;

	readonly subscribeManagement = (listener: Listener): (() => void) => {
		this.#managementListeners.add(listener);
		return () => this.#managementListeners.delete(listener);
	};

	async addConnection(destination: string): Promise<boolean> {
		if (this.#disposed || this.#activeAbortController !== undefined) return false;
		const input = enrollmentInput({ destination });
		if (!input.success) {
			this.#setManagement({ ...this.#managementSnapshot, failure: 'target' });
			return false;
		}
		const abortController = new AbortController();
		this.#activeAbortController = abortController;
		this.#setManagement({
			...this.#managementSnapshot,
			failure: null,
			pending: { destination: input.data.destination, kind: 'add' },
		});
		try {
			const result = await this.#client.addEnvironment(input.data, abortController.signal);
			let success = false;
			let failure: DesktopSshConnectionManagementFailure | null = null;
			switch (result.kind) {
				case 'added':
				case 'existing':
					success = true;
					break;
				case 'cancelled':
					break;
				case 'failed':
				case 'indeterminate':
					failure = managementAddFailure(result);
					break;
			}
			this.#setManagement({
				...this.#managementSnapshot,
				failure,
				pending: null,
			});
			return success;
		} finally {
			this.#clearAbortController(abortController);
		}
	}

	clearFailure(): void {
		if (this.#managementSnapshot.failure === null) return;
		this.#setManagement({ ...this.#managementSnapshot, failure: null });
	}

	connectEnvironment(environmentId: DesktopSshEnvironmentId, environmentName: string): void {
		if (this.#disposed || this.#activeAbortController !== undefined) return;
		this.#setDialog(environmentConnectionState(environmentId, environmentName));
		this.#startEnvironmentConnection(environmentId, environmentName);
	}

	disconnectEnvironment(environmentId: DesktopSshEnvironmentId): void {
		if (this.#disposed || this.#activeAbortController !== undefined) return;
		const abortController = new AbortController();
		this.#activeAbortController = abortController;
		this.#setManagement({
			...this.#managementSnapshot,
			failure: null,
			pending: { environmentId, kind: 'disconnect' },
		});
		void this.#client
			.disconnectEnvironment(environmentId, abortController.signal)
			.then((result) => {
				let failure: DesktopSshConnectionManagementFailure | null = null;
				switch (result.kind) {
					case 'cancelled':
					case 'connected':
					case 'disconnected':
						break;
					case 'failed':
					case 'indeterminate':
						failure = managementConnectionFailure(result);
						break;
				}
				this.#setManagement({
					...this.#managementSnapshot,
					failure,
					pending: null,
				});
			})
			.finally(() => this.#clearAbortController(abortController));
	}

	refreshConnections(): void {
		this.#client.refreshEnvironmentCatalog(true);
	}

	readonly getSnapshot = (): readonly WorkbenchEnvironmentProjectConnectionStatus[] =>
		this.#connectionStatuses;

	readonly subscribe = (listener: Listener): (() => void) => {
		this.#statusListeners.add(listener);
		return () => this.#statusListeners.delete(listener);
	};

	readonly connect = (projectId: string): void => {
		if (this.#disposed || this.#activeAbortController !== undefined) return;
		this.#setDialog({
			canNavigate: false,
			directory: null,
			failure: null,
			kind: 'connection',
			pathInput: '',
			phase: 'working',
			progress: null,
			projectId,
			prompt: null,
		});
		this.#startConnection(projectId);
	};

	readonly disconnect = (projectId: string): void => {
		if (this.#disposed || this.#activeAbortController !== undefined) return;
		const abortController = new AbortController();
		this.#activeAbortController = abortController;
		void this.#client.disconnect(projectId, abortController.signal).finally(() => {
			if (this.#activeAbortController === abortController) {
				this.#activeAbortController = undefined;
			}
		});
	};

	readonly getDialogSnapshot = (): DesktopSshProjectEnrollmentDialogState => this.#dialogState;

	readonly subscribeDialog = (listener: Listener): (() => void) => {
		this.#dialogListeners.add(listener);
		return () => this.#dialogListeners.delete(listener);
	};

	openEnrollment(): void {
		if (this.#disposed || this.#activeAbortController !== undefined) return;
		this.#setDialog(enrollmentState(EMPTY_FORM));
	}

	openFolderAttachment(projectId: string): void {
		if (this.#disposed || this.#activeAbortController !== undefined) return;
		this.#setDialog(attachmentState(projectId, EMPTY_FORM));
	}

	updateForm(update: Partial<DesktopSshProjectEnrollmentForm>): void {
		const state = this.#dialogState;
		if (
			!isDirectoryOperationState(state) ||
			state.phase === 'working' ||
			state.phase === 'prompt' ||
			state.phase === 'directory'
		)
			return;
		const form = { ...state.form, ...update };
		this.#setDialog(
			state.kind === 'enrollment' ? enrollmentState(form) : attachmentState(state.projectId, form),
		);
	}

	updateDirectoryPath(path: string): void {
		const state = this.#dialogState;
		if (!isDirectoryOperationState(state) || state.phase !== 'directory') return;
		this.#setDialog({ ...state, canNavigate: canNavigate(path), pathInput: path });
	}

	navigateDirectory(path: string): void {
		const state = this.#dialogState;
		const deferred = this.#directoryDeferred;
		const parsed = DesktopSshProjectDirectorySelectionResultSchema.safeParse({
			kind: 'navigate',
			path: path.trim(),
		});
		if (
			!isDirectoryOperationState(state) ||
			state.phase !== 'directory' ||
			deferred === undefined ||
			!parsed.success
		) {
			return;
		}
		this.#directoryDeferred = undefined;
		this.#setDialog({
			...state,
			canNavigate: false,
			directory: state.directory === null ? null : { ...state.directory, navigationFailure: null },
			pathInput: path,
			phase: 'working',
		});
		deferred.resolve(parsed.data);
	}

	selectDirectory(): void {
		const state = this.#dialogState;
		const deferred = this.#directoryDeferred;
		if (
			!isDirectoryOperationState(state) ||
			state.phase !== 'directory' ||
			state.directory === null ||
			deferred === undefined
		) {
			return;
		}
		this.#directoryDeferred = undefined;
		this.#setDialog({
			...state,
			canNavigate: false,
			directory: null,
			phase: 'working',
		});
		deferred.resolve({ kind: 'selected' });
	}

	submit(): void {
		const state = this.#dialogState;
		if (
			!isDirectoryOperationState(state) ||
			!state.canSubmit ||
			this.#activeAbortController !== undefined
		) {
			return;
		}
		if (state.kind === 'enrollment') this.#startEnrollment(state.form);
		else this.#startFolderAttachment(state.projectId, state.form);
	}

	retry(): void {
		const state = this.#dialogState;
		if (
			state.kind === 'closed' ||
			state.phase !== 'failed' ||
			this.#activeAbortController !== undefined
		) {
			return;
		}
		if (state.kind === 'enrollment') this.#startEnrollment(state.form);
		else if (state.kind === 'attachment') {
			this.#startFolderAttachment(state.projectId, state.form);
		} else if (state.kind === 'connection') this.#startConnection(state.projectId);
		else this.#startEnvironmentConnection(state.environmentId, state.environmentName);
	}

	resolvePrompt(result: DesktopSshProjectInteractionResult): void {
		const deferred = this.#promptDeferred;
		if (deferred === undefined) return;
		this.#promptDeferred = undefined;
		const state = this.#dialogState;
		if (state.kind !== 'closed') {
			this.#setDialog({ ...state, phase: 'working', prompt: null });
		}
		deferred.resolve(result);
	}

	close(): void {
		if (this.#dialogState.kind === 'closed') return;
		this.#operationGeneration += 1;
		this.#cancelPrompt();
		this.#cancelDirectorySelection();
		this.#activeAbortController?.abort();
		this.#activeAbortController = undefined;
		this.#setDialog({ kind: 'closed' });
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.close();
		this.#unsubscribeStatuses();
		this.#unsubscribeEnvironmentCatalog();
		this.#statusListeners.clear();
		this.#managementListeners.clear();
		this.#dialogListeners.clear();
		this.#client.dispose();
	}

	#startEnrollment(form: DesktopSshProjectEnrollmentForm): void {
		const parsed = enrollmentInput(form);
		if (!parsed.success) return;
		const generation = ++this.#operationGeneration;
		const abortController = new AbortController();
		this.#activeAbortController = abortController;
		this.#setDialog({
			...enrollmentState(form),
			phase: 'working',
			progress: 'resolving-target',
		});
		void this.#client
			.enroll(
				parsed.data,
				{
					interact: (prompt) => this.#interact(generation, prompt),
					progress: (progress) => {
						if (this.#operationGeneration !== generation) return;
						const state = this.#dialogState;
						if (state.kind === 'enrollment') this.#setDialog({ ...state, progress });
					},
					selectDirectory: (snapshot) => this.#chooseDirectory(generation, snapshot),
				},
				abortController.signal,
			)
			.then((result) => this.#settleEnrollment(generation, result))
			.finally(() => this.#clearAbortController(abortController));
	}

	#startFolderAttachment(projectId: string, form: DesktopSshProjectEnrollmentForm): void {
		const parsed = enrollmentInput(form);
		if (!parsed.success) return;
		const generation = ++this.#operationGeneration;
		const abortController = new AbortController();
		this.#activeAbortController = abortController;
		this.#setDialog({
			...attachmentState(projectId, form),
			phase: 'working',
			progress: 'resolving-target',
		});
		void this.#client
			.attachFolder(
				projectId,
				parsed.data,
				{
					interact: (prompt) => this.#interact(generation, prompt),
					progress: (progress) => {
						if (this.#operationGeneration !== generation) return;
						const state = this.#dialogState;
						if (state.kind === 'attachment') this.#setDialog({ ...state, progress });
					},
					selectDirectory: (snapshot) => this.#chooseDirectory(generation, snapshot),
				},
				abortController.signal,
			)
			.then((result) => this.#settleFolderAttachment(generation, result))
			.finally(() => this.#clearAbortController(abortController));
	}

	#startConnection(projectId: string): void {
		const generation = ++this.#operationGeneration;
		const abortController = new AbortController();
		this.#activeAbortController = abortController;
		this.#setDialog({
			canNavigate: false,
			directory: null,
			failure: null,
			kind: 'connection',
			pathInput: '',
			phase: 'working',
			progress: null,
			projectId,
			prompt: null,
		});
		void this.#client
			.connect(
				projectId,
				{ interact: (prompt) => this.#interact(generation, prompt) },
				abortController.signal,
			)
			.then((result) => this.#settleConnection(generation, result))
			.finally(() => this.#clearAbortController(abortController));
	}

	#startEnvironmentConnection(
		environmentId: DesktopSshEnvironmentId,
		environmentName: string,
	): void {
		const generation = ++this.#operationGeneration;
		const abortController = new AbortController();
		this.#activeAbortController = abortController;
		this.#setDialog(environmentConnectionState(environmentId, environmentName, 'working'));
		void this.#client
			.connectEnvironment(
				environmentId,
				{ interact: (prompt) => this.#interact(generation, prompt) },
				abortController.signal,
			)
			.then((result) => this.#settleEnvironmentConnection(generation, result))
			.finally(() => this.#clearAbortController(abortController));
	}

	#chooseDirectory(
		generation: number,
		snapshot: DesktopSshProjectDirectorySnapshot,
	): Promise<DesktopSshProjectDirectorySelectionResult> {
		if (
			this.#disposed ||
			this.#operationGeneration !== generation ||
			this.#directoryDeferred !== undefined ||
			this.#promptDeferred !== undefined
		) {
			return Promise.resolve({ kind: 'cancelled' });
		}
		const state = this.#dialogState;
		if (!isDirectoryOperationState(state)) return Promise.resolve({ kind: 'cancelled' });
		const deferred = createDeferred<DesktopSshProjectDirectorySelectionResult>();
		this.#directoryDeferred = deferred;
		const pathInput =
			snapshot.navigationFailure === null ? snapshot.canonicalPath : state.pathInput;
		this.#setDialog({
			...state,
			canNavigate: canNavigate(pathInput),
			directory: snapshot,
			pathInput,
			phase: 'directory',
			prompt: null,
		});
		return deferred.promise;
	}

	#interact(
		generation: number,
		prompt: DesktopSshProjectInteractionPrompt,
	): Promise<DesktopSshProjectInteractionResult> {
		if (
			this.#disposed ||
			this.#operationGeneration !== generation ||
			this.#promptDeferred !== undefined ||
			this.#directoryDeferred !== undefined
		) {
			return Promise.resolve({ kind: 'cancelled' });
		}
		const state = this.#dialogState;
		if (state.kind === 'closed') return Promise.resolve({ kind: 'cancelled' });
		const deferred = createDeferred<DesktopSshProjectInteractionResult>();
		this.#promptDeferred = deferred;
		this.#setDialog({ ...state, phase: 'prompt', prompt });
		return deferred.promise;
	}

	async #settleEnrollment(
		generation: number,
		result: DesktopSshProjectEnrollmentResult,
	): Promise<void> {
		if (this.#operationGeneration !== generation || this.#dialogState.kind !== 'enrollment') return;
		this.#cancelPrompt();
		this.#cancelDirectorySelection();
		if ('canonicalPath' in result) {
			const activated = await this.#onEnrollmentCommitted(result).catch(() => false);
			if (this.#operationGeneration !== generation || this.#dialogState.kind !== 'enrollment') {
				return;
			}
			if (activated) this.#setDialog({ kind: 'closed' });
			else {
				this.#setDialog({
					...this.#dialogState,
					failure: 'workbench',
					phase: 'failed',
					progress: null,
					prompt: null,
				});
			}
			return;
		}
		if (result.kind === 'cancelled') {
			this.#setDialog({ kind: 'closed' });
			return;
		}
		this.#setDialog({
			...this.#dialogState,
			failure:
				result.kind === 'indeterminate' ? 'control' : enrollmentFailureCategory(result.reason),
			phase: 'failed',
			progress: null,
			prompt: null,
		});
	}

	async #settleFolderAttachment(
		generation: number,
		result: DesktopSshProjectFolderAttachmentResult,
	): Promise<void> {
		if (this.#operationGeneration !== generation || this.#dialogState.kind !== 'attachment') {
			return;
		}
		this.#cancelPrompt();
		this.#cancelDirectorySelection();
		if ('canonicalPath' in result) {
			const published = await this.#onFolderAttachmentCommitted(result).catch(() => false);
			if (this.#operationGeneration !== generation || this.#dialogState.kind !== 'attachment') {
				return;
			}
			if (published) this.#setDialog({ kind: 'closed' });
			else {
				this.#setDialog({
					...this.#dialogState,
					failure: 'workbench',
					phase: 'failed',
					progress: null,
					prompt: null,
				});
			}
			return;
		}
		if (result.kind === 'cancelled') {
			this.#setDialog({ kind: 'closed' });
			return;
		}
		this.#setDialog({
			...this.#dialogState,
			failure:
				result.kind === 'indeterminate' ? 'control' : attachmentFailureCategory(result.reason),
			phase: 'failed',
			progress: null,
			prompt: null,
		});
	}

	#settleConnection(generation: number, result: DesktopSshProjectConnectionResult): void {
		if (this.#operationGeneration !== generation || this.#dialogState.kind !== 'connection') return;
		this.#cancelPrompt();
		this.#cancelDirectorySelection();
		if ('projectId' in result || result.kind === 'cancelled') {
			this.#setDialog({ kind: 'closed' });
			return;
		}
		this.#setDialog({
			...this.#dialogState,
			failure:
				result.kind === 'indeterminate' ? 'control' : connectionFailureCategory(result.reason),
			phase: 'failed',
			progress: null,
			prompt: null,
		});
	}

	#settleEnvironmentConnection(
		generation: number,
		result: DesktopSshEnvironmentConnectionResult,
	): void {
		if (
			this.#operationGeneration !== generation ||
			this.#dialogState.kind !== 'environment-connection'
		) {
			return;
		}
		this.#cancelPrompt();
		this.#cancelDirectorySelection();
		if ('environmentId' in result || result.kind === 'cancelled') {
			this.#setDialog({ kind: 'closed' });
			return;
		}
		this.#setDialog({
			...this.#dialogState,
			failure:
				result.kind === 'indeterminate'
					? 'control'
					: environmentConnectionFailureCategory(result.reason),
			phase: 'failed',
			progress: null,
			prompt: null,
		});
	}

	#clearAbortController(abortController: AbortController): void {
		if (this.#activeAbortController === abortController) this.#activeAbortController = undefined;
	}

	#cancelPrompt(): void {
		this.#promptDeferred?.resolve({ kind: 'cancelled' });
		this.#promptDeferred = undefined;
	}

	#cancelDirectorySelection(): void {
		this.#directoryDeferred?.resolve({ kind: 'cancelled' });
		this.#directoryDeferred = undefined;
	}

	#setDialog(state: DesktopSshProjectEnrollmentDialogState): void {
		this.#dialogState = state;
		for (const listener of this.#dialogListeners) listener();
	}

	#setManagement(snapshot: DesktopSshConnectionManagementSnapshot): void {
		this.#managementSnapshot = snapshot;
		for (const listener of this.#managementListeners) listener();
	}
}

function enrollmentState(
	form: DesktopSshProjectEnrollmentForm,
): Extract<DesktopSshProjectEnrollmentDialogState, { readonly kind: 'enrollment' }> {
	return {
		canNavigate: false,
		canSubmit: enrollmentInput(form).success,
		directory: null,
		failure: null,
		form,
		kind: 'enrollment',
		pathInput: '',
		phase: 'editing',
		progress: null,
		prompt: null,
	};
}

function attachmentState(
	projectId: string,
	form: DesktopSshProjectEnrollmentForm,
): Extract<DesktopSshProjectEnrollmentDialogState, { readonly kind: 'attachment' }> {
	return {
		canNavigate: false,
		canSubmit: enrollmentInput(form).success,
		directory: null,
		failure: null,
		form,
		kind: 'attachment',
		pathInput: '',
		phase: 'editing',
		progress: null,
		projectId,
		prompt: null,
	};
}

function environmentConnectionState(
	environmentId: DesktopSshEnvironmentId,
	environmentName: string,
	phase: 'failed' | 'working' = 'working',
): Extract<DesktopSshProjectEnrollmentDialogState, { readonly kind: 'environment-connection' }> {
	return {
		canNavigate: false,
		directory: null,
		environmentId,
		environmentName,
		failure: null,
		kind: 'environment-connection',
		pathInput: '',
		phase,
		progress: null,
		prompt: null,
	};
}

function isDirectoryOperationState(
	state: DesktopSshProjectEnrollmentDialogState,
): state is Extract<
	DesktopSshProjectEnrollmentDialogState,
	{ readonly kind: 'attachment' | 'enrollment' }
> {
	return state.kind === 'attachment' || state.kind === 'enrollment';
}

function enrollmentInput(form: DesktopSshProjectEnrollmentForm) {
	return DesktopSshProjectEnrollmentInputSchema.safeParse({
		destination: form.destination.trim(),
	});
}

function canNavigate(path: string): boolean {
	return DesktopSshProjectDirectorySelectionResultSchema.safeParse({
		kind: 'navigate',
		path: path.trim(),
	}).success;
}

function mapConnectionStatuses(
	statuses: ReturnType<DesktopSshProjectEnrollmentClient['getStatuses']>,
): readonly WorkbenchEnvironmentProjectConnectionStatus[] {
	return statuses.map((status) => ({ projectId: status.projectId, state: status.state }));
}

function enrollmentFailureCategory(
	reason: DesktopSshProjectEnrollmentFailureReason,
): DesktopSshProjectEnrollmentFailureCategory {
	switch (reason) {
		case 'busy':
			return 'busy';
		case 'local-platform-unsupported':
			return 'unsupported';
		case 'target-resolution-failed':
			return 'target';
		case 'environment-capacity':
		case 'environment-conflict':
		case 'registry-unavailable':
			return 'storage';
		case 'project-root-busy':
		case 'project-root-invalid':
		case 'project-root-not-directory':
		case 'project-root-not-found':
		case 'project-root-permission-denied':
		case 'project-root-timed-out':
			return 'project-path';
		case 'workbench-missing':
		case 'workbench-unavailable':
			return 'workbench';
		case 'connection-owner-unavailable':
		case 'environment-not-found':
		case 'remote-runtime-unavailable':
			return 'connection';
	}
}

function attachmentFailureCategory(
	reason: DesktopSshProjectFolderAttachmentFailureReason,
): DesktopSshProjectEnrollmentFailureCategory {
	switch (reason) {
		case 'project-environment-conflict':
		case 'project-folder-already-owned':
			return 'project-attachment';
		case 'project-not-found':
			return 'workbench';
		default:
			return enrollmentFailureCategory(reason);
	}
}

function connectionFailureCategory(
	reason: Extract<DesktopSshProjectConnectionResult, { readonly kind: 'failed' }>['reason'],
): DesktopSshProjectEnrollmentFailureCategory {
	switch (reason) {
		case 'busy':
			return 'busy';
		case 'local-platform-unsupported':
			return 'unsupported';
		case 'registry-unavailable':
			return 'storage';
		case 'workbench-missing':
		case 'workbench-unavailable':
			return 'workbench';
		case 'connection-owner-unavailable':
		case 'environment-not-found':
		case 'project-not-found':
		case 'remote-runtime-unavailable':
			return 'connection';
	}
}

function environmentConnectionFailureCategory(
	reason: Extract<DesktopSshEnvironmentConnectionResult, { readonly kind: 'failed' }>['reason'],
): DesktopSshProjectEnrollmentFailureCategory {
	switch (reason) {
		case 'busy':
			return 'busy';
		case 'local-platform-unsupported':
			return 'unsupported';
		case 'registry-unavailable':
			return 'storage';
		case 'connection-owner-unavailable':
		case 'environment-not-found':
		case 'remote-runtime-unavailable':
			return 'connection';
	}
}

function managementAddFailure(
	result: Exclude<
		DesktopSshEnvironmentAddResult,
		{ readonly kind: 'added' | 'cancelled' | 'existing' }
	>,
): DesktopSshConnectionManagementFailure {
	if (result.kind === 'indeterminate') return 'control';
	switch (result.reason) {
		case 'busy':
			return 'busy';
		case 'target-resolution-failed':
			return 'target';
		case 'environment-capacity':
		case 'environment-conflict':
		case 'registry-unavailable':
			return 'storage';
	}
}

function managementConnectionFailure(
	result: Exclude<
		DesktopSshEnvironmentConnectionResult,
		{ readonly kind: 'cancelled' | 'connected' | 'disconnected' }
	>,
): DesktopSshConnectionManagementFailure {
	if (result.kind === 'indeterminate') return 'control';
	if (result.reason === 'busy') return 'busy';
	if (result.reason === 'registry-unavailable') return 'storage';
	return 'connection';
}

function createDeferred<Value>(): {
	readonly promise: Promise<Value>;
	readonly resolve: (value: Value) => void;
} {
	let resolvePromise: (value: Value) => void = () => undefined;
	const promise = new Promise<Value>((resolve) => {
		resolvePromise = resolve;
	});
	return { promise, resolve: resolvePromise };
}
