import {
	DesktopProjectWorktreeMainEventSchema,
	DesktopProjectWorktreeRendererCommandSchema,
	DesktopProjectWorktreeRequestIdSchema,
	type DesktopProjectWorktreeRequestId,
} from '@orchbee/desktop-client-protocol';
import type {
	WorkbenchProjectWorktreeCapability,
	WorkbenchProjectWorktreeCreationResult,
	WorkbenchProjectWorktreeDiscoveryResult,
	WorkbenchProjectWorktreeFailureCode,
} from '@orchbee/workbench';

type PendingRequest =
	| {
			readonly kind: 'creation';
			readonly requestId: DesktopProjectWorktreeRequestId;
			resolve(result: WorkbenchProjectWorktreeCreationResult): void;
	  }
	| {
			readonly kind: 'discovery';
			readonly requestId: DesktopProjectWorktreeRequestId;
			resolve(result: WorkbenchProjectWorktreeDiscoveryResult): void;
	  };

export class DesktopProjectWorktreeClient implements WorkbenchProjectWorktreeCapability {
	readonly #createRequestId: () => DesktopProjectWorktreeRequestId;
	#disposed = false;
	#pending: PendingRequest | undefined;
	readonly #port: MessagePort;
	#sequence = 0;

	constructor(
		port: MessagePort,
		createRequestId?: (() => DesktopProjectWorktreeRequestId) | undefined,
	) {
		this.#port = port;
		this.#createRequestId =
			createRequestId ??
			(() =>
				DesktopProjectWorktreeRequestIdSchema.parse(
					`desktop-project-worktree:${++this.#sequence}`,
				));
		this.#port.addEventListener('message', this.#handleMessage);
		this.#port.start();
	}

	create(
		input: Parameters<WorkbenchProjectWorktreeCapability['create']>[0],
	): Promise<WorkbenchProjectWorktreeCreationResult> {
		if (this.#disposed) return Promise.resolve(failed('control-unavailable'));
		if (this.#pending !== undefined) return Promise.resolve(failed('busy'));
		const requestId = this.#createRequestId();
		const result = new Promise<WorkbenchProjectWorktreeCreationResult>((resolve) => {
			this.#pending = { kind: 'creation', requestId, resolve };
		});
		try {
			this.#port.postMessage(
				DesktopProjectWorktreeRendererCommandSchema.parse({
					...input,
					requestId,
					type: 'desktop-project-worktree-creation-requested',
				}),
			);
		} catch {
			this.#settleFailed('control-unavailable');
		}
		return result;
	}

	discover(
		input: Parameters<WorkbenchProjectWorktreeCapability['discover']>[0],
	): Promise<WorkbenchProjectWorktreeDiscoveryResult> {
		if (this.#disposed) return Promise.resolve(failed('control-unavailable'));
		if (this.#pending !== undefined) return Promise.resolve(failed('busy'));
		const requestId = this.#createRequestId();
		const result = new Promise<WorkbenchProjectWorktreeDiscoveryResult>((resolve) => {
			this.#pending = { kind: 'discovery', requestId, resolve };
		});
		try {
			this.#port.postMessage(
				DesktopProjectWorktreeRendererCommandSchema.parse({
					...input,
					requestId,
					type: 'desktop-project-worktree-discovery-requested',
				}),
			);
		} catch {
			this.#settleFailed('control-unavailable');
		}
		return result;
	}

	dispose(): void {
		if (this.#disposed) return;
		this.#disposed = true;
		this.#port.removeEventListener('message', this.#handleMessage);
		this.#settleFailed('control-unavailable');
		this.#port.close();
	}

	readonly #handleMessage = (event: MessageEvent<unknown>): void => {
		if (this.#disposed || event.ports.length !== 0) {
			this.dispose();
			return;
		}
		const parsed = DesktopProjectWorktreeMainEventSchema.safeParse(event.data);
		if (!parsed.success) {
			this.dispose();
			return;
		}
		const pending = this.#pending;
		if (pending === undefined || pending.requestId !== parsed.data.requestId) return;
		this.#pending = undefined;
		if (parsed.data.type === 'desktop-project-worktree-failed') {
			pending.resolve(failed(parsed.data.code));
			return;
		}
		if (
			parsed.data.type === 'desktop-project-worktree-discovery-completed' &&
			pending.kind === 'discovery'
		) {
			pending.resolve({ discovery: parsed.data.discovery, kind: 'discovered' });
			return;
		}
		if (
			parsed.data.type === 'desktop-project-worktree-creation-completed' &&
			pending.kind === 'creation'
		) {
			pending.resolve({ ...parsed.data.result, kind: 'created' });
			return;
		}
		this.dispose();
	};

	#settleFailed(code: WorkbenchProjectWorktreeFailureCode): void {
		const pending = this.#pending;
		this.#pending = undefined;
		pending?.resolve(failed(code));
	}
}

function failed(code: WorkbenchProjectWorktreeFailureCode) {
	return { code, kind: 'failed' } as const;
}
