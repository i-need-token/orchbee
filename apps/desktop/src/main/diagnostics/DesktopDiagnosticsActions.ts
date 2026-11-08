import {
	ActionRuntime,
	createTypedActionRuntime,
	defineAction,
	noActionInput,
	type ActionAvailability,
	type ActionDisposable,
	type ActionInvocationSource,
	type ActionResult,
	type ActionSignature,
} from '@orchbee/actions';
import { desktopDiagnosticsMessages } from '@orchbee/i18n';

const SAVE_DIAGNOSTIC_BUNDLE_ACTION_ID = 'desktop.diagnostics.save-bundle';

interface DesktopDiagnosticsActionCatalog {
	readonly [SAVE_DIAGNOSTIC_BUNDLE_ACTION_ID]: ActionSignature<void>;
}

/**
 * Why a save failed: the `write-failed` stage `orchbee diagnostics create` reports, a command that
 * failed otherwise, or an answer that could not be read. A log record may hold it.
 */
export type DesktopDiagnosticBundleFailureCode =
	| 'client-failed'
	| 'create-temporary'
	| 'destination'
	| 'publish'
	| 'sync-temporary'
	| 'unreadable-result'
	| 'write-temporary';

/** How one `orchbee diagnostics create` run ended. */
export type DesktopDiagnosticBundleOutcome =
	| { readonly kind: 'created'; readonly path: string }
	| { readonly code: DesktopDiagnosticBundleFailureCode; readonly kind: 'failed' };

export interface DesktopDiagnosticsTarget {
	/** The file the person chose in the save dialog, or `undefined` when they cancelled it. */
	chooseBundlePath(): Promise<string | undefined>;
	createBundle(path: string): Promise<DesktopDiagnosticBundleOutcome>;
	reportFailure(): Promise<void>;
	revealBundle(path: string): void;
}

export type DesktopDiagnosticsActionSource = Extract<ActionInvocationSource, 'menu'>;

/**
 * Saves a diagnostic bundle where the person chooses through the CLI's `diagnostics create`, which
 * owns its contents and redaction. One save runs at a time; the bundle is shown in its folder when
 * it is written, and a failure says so in a dialog. The bundle command is Windows only until
 * ORC-328, so the Action is hidden elsewhere.
 */
export class DesktopDiagnosticsActions {
	readonly #platform: NodeJS.Platform;
	readonly #registrations: readonly ActionDisposable[];
	readonly #runtime = new ActionRuntime();
	readonly #scope = this.#runtime.createScope('desktop.diagnostics');
	readonly #target: DesktopDiagnosticsTarget;
	readonly #typedRuntime = createTypedActionRuntime<DesktopDiagnosticsActionCatalog>(this.#runtime);
	#saving = false;

	constructor(target: DesktopDiagnosticsTarget, platform: NodeJS.Platform) {
		this.#target = target;
		this.#platform = platform;
		this.#registrations = [
			this.#typedRuntime.register(
				this.#scope,
				defineAction({
					category: { id: 'help', title: desktopDiagnosticsMessages.category },
					execution: {
						concurrency: 'reject-duplicate',
						concurrencyKey: () => SAVE_DIAGNOSTIC_BUNDLE_ACTION_ID,
						mode: 'foreground',
					},
					getAvailability: () => this.#availability(),
					handler: {
						execute: async (context) => {
							this.#saving = true;
							try {
								return await this.#save(() => context.markCommitted());
							} finally {
								this.#saving = false;
							}
						},
					},
					id: SAVE_DIAGNOSTIC_BUNDLE_ACTION_ID,
					input: noActionInput,
					title: desktopDiagnosticsMessages.saveBundle,
				}),
			),
		];
	}

	dispose(): void {
		for (const registration of this.#registrations) registration.dispose();
		this.#runtime.dispose();
	}

	getSaveBundleAvailability(): ActionAvailability {
		return this.#availability();
	}

	/** Starts a save, or returns `false` when the Action is not available now. */
	invokeSaveBundle(source: DesktopDiagnosticsActionSource): boolean {
		const invocation = { scope: this.#scope, source };
		const resolved = this.#typedRuntime.resolveAction(
			SAVE_DIAGNOSTIC_BUNDLE_ACTION_ID,
			undefined,
			invocation,
		);
		if (resolved?.availability.kind !== 'enabled') return false;
		void this.#typedRuntime.execute(SAVE_DIAGNOSTIC_BUNDLE_ACTION_ID, undefined, invocation)
			.completion;
		return true;
	}

	#availability(): ActionAvailability {
		if (this.#platform !== 'win32') return { kind: 'hidden' };
		return this.#saving
			? { kind: 'disabled', reason: desktopDiagnosticsMessages.bundlePending }
			: { kind: 'enabled' };
	}

	async #save(markCommitted: () => void): Promise<ActionResult<void>> {
		const path = await this.#target.chooseBundlePath();
		if (path === undefined) return { kind: 'cancelled' };
		markCommitted();
		const outcome = await this.#target.createBundle(path);
		if (outcome.kind === 'created') {
			this.#target.revealBundle(outcome.path);
			return { kind: 'completed' };
		}
		await this.#target.reportFailure();
		return { failure: { code: outcome.code }, kind: 'failed' };
	}
}
