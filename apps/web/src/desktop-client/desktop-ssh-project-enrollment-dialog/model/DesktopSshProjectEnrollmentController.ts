import {
	DesktopSshProjectEnrollmentInputSchema,
	type DesktopSshProjectConnectionResult,
	type DesktopSshProjectEnrollmentFailureReason,
	type DesktopSshProjectEnrollmentProgressStage,
	type DesktopSshProjectEnrollmentResult,
	type DesktopSshProjectInteractionResult,
} from '@orchbee/desktop-client-protocol';
import type {
	WorkbenchEnvironmentProjectConnectionCapability,
	WorkbenchEnvironmentProjectConnectionStatus,
} from '@orchbee/workbench';

import type {
	DesktopSshProjectEnrollmentClient,
	DesktopSshProjectInteractionPrompt,
} from './DesktopSshProjectEnrollmentClient';

export interface DesktopSshProjectEnrollmentForm {
	readonly configurationFilePath: string;
	readonly destination: string;
	readonly name: string;
	readonly projectPath: string;
}

export type DesktopSshProjectEnrollmentFailureCategory =
	| 'busy'
	| 'connection'
	| 'control'
	| 'project-path'
	| 'storage'
	| 'target'
	| 'unsupported'
	| 'workbench';

interface DesktopSshProjectEnrollmentDialogBase {
	readonly failure: DesktopSshProjectEnrollmentFailureCategory | null;
	readonly phase: 'editing' | 'failed' | 'prompt' | 'working';
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
			readonly kind: 'connection';
			readonly projectId: string;
	  });

export interface DesktopSshProjectEnrollmentControllerOptions {
	readonly client: DesktopSshProjectEnrollmentControllerClient;
	readonly onEnrollmentCommitted: (
		result: Extract<DesktopSshProjectEnrollmentResult, { readonly kind: 'created' | 'existing' }>,
	) => Promise<boolean>;
}

export interface DesktopSshProjectEnrollmentControllerClient {
	connect: DesktopSshProjectEnrollmentClient['connect'];
	disconnect: DesktopSshProjectEnrollmentClient['disconnect'];
	dispose: DesktopSshProjectEnrollmentClient['dispose'];
	enroll: DesktopSshProjectEnrollmentClient['enroll'];
	getStatuses: DesktopSshProjectEnrollmentClient['getStatuses'];
	subscribeStatuses: DesktopSshProjectEnrollmentClient['subscribeStatuses'];
}

export interface DesktopSshProjectEnrollmentDialogController {
	close(): void;
	getDialogSnapshot(): DesktopSshProjectEnrollmentDialogState;
	resolvePrompt(result: DesktopSshProjectInteractionResult): void;
	retry(): void;
	submit(): void;
	subscribeDialog(listener: Listener): () => void;
	updateForm(update: Partial<DesktopSshProjectEnrollmentForm>): void;
}

type Listener = () => void;

const EMPTY_FORM: DesktopSshProjectEnrollmentForm = {
	configurationFilePath: '',
	destination: '',
	name: '',
	projectPath: '',
};

export class DesktopSshProjectEnrollmentController implements WorkbenchEnvironmentProjectConnectionCapability {
	#activeAbortController: AbortController | undefined;
	readonly #client: DesktopSshProjectEnrollmentControllerClient;
	#connectionStatuses: readonly WorkbenchEnvironmentProjectConnectionStatus[];
	#dialogState: DesktopSshProjectEnrollmentDialogState = { kind: 'closed' };
	readonly #dialogListeners = new Set<Listener>();
	#disposed = false;
	readonly #onEnrollmentCommitted: DesktopSshProjectEnrollmentControllerOptions['onEnrollmentCommitted'];
	#operationGeneration = 0;
	#promptDeferred:
		| ReturnType<typeof createDeferred<DesktopSshProjectInteractionResult>>
		| undefined;
	readonly #statusListeners = new Set<Listener>();
	readonly #unsubscribeStatuses: () => void;

	constructor(options: DesktopSshProjectEnrollmentControllerOptions) {
		this.#client = options.client;
		this.#onEnrollmentCommitted = options.onEnrollmentCommitted;
		this.#connectionStatuses = mapConnectionStatuses(options.client.getStatuses());
		this.#unsubscribeStatuses = options.client.subscribeStatuses(() => {
			this.#connectionStatuses = mapConnectionStatuses(this.#client.getStatuses());
			for (const listener of this.#statusListeners) listener();
		});
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
			failure: null,
			kind: 'connection',
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

	updateForm(update: Partial<DesktopSshProjectEnrollmentForm>): void {
		const state = this.#dialogState;
		if (state.kind !== 'enrollment' || state.phase === 'working' || state.phase === 'prompt')
			return;
		this.#setDialog(enrollmentState({ ...state.form, ...update }));
	}

	submit(): void {
		const state = this.#dialogState;
		if (
			state.kind !== 'enrollment' ||
			!state.canSubmit ||
			this.#activeAbortController !== undefined
		) {
			return;
		}
		this.#startEnrollment(state.form);
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
		else if (state.kind === 'connection') this.#startConnection(state.projectId);
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
		this.#activeAbortController?.abort();
		this.#activeAbortController = undefined;
		this.#setDialog({ kind: 'closed' });
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.close();
		this.#unsubscribeStatuses();
		this.#statusListeners.clear();
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
				},
				abortController.signal,
			)
			.then((result) => this.#settleEnrollment(generation, result))
			.finally(() => this.#clearAbortController(abortController));
	}

	#startConnection(projectId: string): void {
		const generation = ++this.#operationGeneration;
		const abortController = new AbortController();
		this.#activeAbortController = abortController;
		this.#setDialog({
			failure: null,
			kind: 'connection',
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

	#interact(
		generation: number,
		prompt: DesktopSshProjectInteractionPrompt,
	): Promise<DesktopSshProjectInteractionResult> {
		if (
			this.#disposed ||
			this.#operationGeneration !== generation ||
			this.#promptDeferred !== undefined
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

	#settleConnection(generation: number, result: DesktopSshProjectConnectionResult): void {
		if (this.#operationGeneration !== generation || this.#dialogState.kind !== 'connection') return;
		this.#cancelPrompt();
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

	#clearAbortController(abortController: AbortController): void {
		if (this.#activeAbortController === abortController) this.#activeAbortController = undefined;
	}

	#cancelPrompt(): void {
		this.#promptDeferred?.resolve({ kind: 'cancelled' });
		this.#promptDeferred = undefined;
	}

	#setDialog(state: DesktopSshProjectEnrollmentDialogState): void {
		this.#dialogState = state;
		for (const listener of this.#dialogListeners) listener();
	}
}

function enrollmentState(
	form: DesktopSshProjectEnrollmentForm,
): Extract<DesktopSshProjectEnrollmentDialogState, { readonly kind: 'enrollment' }> {
	return {
		canSubmit: enrollmentInput(form).success,
		failure: null,
		form,
		kind: 'enrollment',
		phase: 'editing',
		progress: null,
		prompt: null,
	};
}

function enrollmentInput(form: DesktopSshProjectEnrollmentForm) {
	return DesktopSshProjectEnrollmentInputSchema.safeParse({
		configurationFilePath:
			form.configurationFilePath.trim().length === 0 ? null : form.configurationFilePath.trim(),
		destination: form.destination.trim(),
		name: form.name.trim(),
		projectPath: form.projectPath.trim(),
	});
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
