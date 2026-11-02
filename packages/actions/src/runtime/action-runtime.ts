import type {
	ActionAvailability,
	ActionAvailabilityContext,
	ActionConcurrency,
	ActionConfirmation,
	ActionDefinition,
	ActionDescriptor,
	ActionDiscoveryOptions,
	ActionDiscoveryResult,
	ActionDisposable,
	ActionExecution,
	ActionExecutionContext,
	ActionExecutionId,
	ActionExecutionSnapshot,
	ActionInputValidation,
	ActionInvocation,
	ActionProgress,
	ActionResult,
	ActionRuntimeOptions,
	ActionScope,
	ActionValidationResult,
	ActionConfirmationPresenter,
	ActionCatalogId,
	ActionInputFor,
	ActionOutputFor,
	CatalogActionDefinition,
	PreparedActionInvocation,
	ResolvedAction,
	TypedActionRuntime,
	UnboundActionInvocation,
} from './action-runtime-types';

interface StoredAction {
	readonly automation: 'allowed' | 'denied';
	readonly catalogToken: object | null;
	readonly concurrency: ActionConcurrency;
	readonly descriptor: ActionDescriptor;
	readonly inputName: string;
	getDiscoveryAvailability(context: ActionAvailabilityContext): ActionAvailability | null;
	parse(input: unknown): ActionValidationResult<ParsedActionInput>;
}

interface ConcurrencyEntry {
	active: ManagedActionExecution<unknown> | null;
	draining: boolean;
	readonly queue: Array<() => void>;
}

interface ParsedActionInput {
	concurrencyKey(context: ActionAvailabilityContext): string;
	execute(context: ActionExecutionContext): ActionResult | Promise<ActionResult>;
	getAvailability(context: ActionAvailabilityContext): ActionAvailability;
	getConfirmation(context: ActionAvailabilityContext): ActionConfirmation | null;
}

interface ActionInvocationBinding {
	readonly action: StoredAction;
	readonly actionId: string;
	readonly invocation: ActionInvocation;
	readonly scope: ActionScope;
}

interface PreparedInvocationBinding extends ActionInvocationBinding {
	readonly input: ParsedActionInput;
}

const TERMINAL_STATES = new Set([
	'blocked',
	'cancelled',
	'completed',
	'failed',
	'unchanged',
	'unsupported',
]);

function isPromiseLike<Value>(value: Value | Promise<Value>): value is Promise<Value> {
	return (
		value !== null &&
		(typeof value === 'object' || typeof value === 'function') &&
		'then' in value &&
		typeof value.then === 'function'
	);
}

export const noActionInput: ActionInputValidation<void> = {
	name: 'void',
	parse(input): ActionValidationResult<void> {
		return input === undefined
			? { success: true, data: undefined }
			: { success: false, code: 'expected-void' };
	},
};

export function defineAction<Input, Output = void, const Id extends string = string>(
	definition: ActionDefinition<Input, Output> & { readonly id: Id },
): ActionDefinition<Input, Output> & { readonly id: Id } {
	return definition;
}

class ManagedActionExecution<Output> implements ActionExecution<Output> {
	public readonly completion: Promise<ActionResult<Output>>;
	public readonly executionId: ActionExecutionId;
	public readonly signal: AbortSignal;

	private readonly abortController = new AbortController();
	private readonly listeners = new Set<() => void>();
	private committed = false;
	private handlerStarted = false;
	private resolveCompletion: ((result: ActionResult<Output>) => void) | null = null;
	private snapshot: ActionExecutionSnapshot<Output> = {
		progress: null,
		result: null,
		state: 'resolving',
	};

	public constructor(executionId: ActionExecutionId) {
		this.executionId = executionId;
		this.signal = this.abortController.signal;
		this.completion = new Promise((resolve) => {
			this.resolveCompletion = resolve;
		});
	}

	public cancel(): void {
		if (this.isTerminal()) {
			return;
		}
		this.abortController.abort();
		if (!this.handlerStarted) {
			this.settle({ kind: 'cancelled' });
			return;
		}
		this.update({ ...this.snapshot, state: 'cancelling' });
	}

	public getSnapshot(): ActionExecutionSnapshot<Output> {
		return this.snapshot;
	}

	public subscribe(listener: () => void): ActionDisposable {
		this.listeners.add(listener);
		return {
			dispose: () => {
				this.listeners.delete(listener);
			},
		};
	}

	public isCommitted(): boolean {
		return this.committed;
	}

	public isTerminal(): boolean {
		return TERMINAL_STATES.has(this.snapshot.state);
	}

	public markCommitted(): void {
		this.committed = true;
	}

	public markHandlerStarted(): void {
		this.handlerStarted = true;
	}

	public reportProgress(progress: ActionProgress): void {
		if (this.isTerminal()) {
			return;
		}
		this.update({ ...this.snapshot, progress });
	}

	public setState(state: ActionExecutionSnapshot<Output>['state']): void {
		if (this.isTerminal()) {
			return;
		}
		this.update({ ...this.snapshot, state });
	}

	public settle(result: ActionResult<Output>): void {
		if (this.isTerminal()) {
			return;
		}
		this.update({
			progress: this.snapshot.progress,
			result,
			state: result.kind,
		});
		this.resolveCompletion?.(result);
		this.resolveCompletion = null;
	}

	private update(snapshot: ActionExecutionSnapshot<Output>): void {
		this.snapshot = snapshot;
		for (const listener of new Set(this.listeners)) {
			if (!this.listeners.has(listener)) continue;
			try {
				listener();
			} catch {
				// An observer never owns or alters the Action execution lifecycle.
				continue;
			}
		}
	}
}

export class ActionRuntime implements ActionDisposable {
	private readonly concurrencyEntries = new Map<string, ConcurrencyEntry>();
	private readonly confirmationPresenters = new Map<ActionScope, ActionConfirmationPresenter>();
	private readonly createExecutionId: () => ActionExecutionId;
	private readonly executions = new Set<ManagedActionExecution<unknown>>();
	private readonly listeners = new Set<() => void>();
	private readonly options: ActionRuntimeOptions;
	private readonly preparedInvocations = new WeakMap<
		PreparedActionInvocation,
		PreparedInvocationBinding
	>();
	private readonly registrations = new Map<ActionScope, Map<string, StoredAction>>();
	private readonly scopeOrdinals = new WeakMap<ActionScope, number>();
	private readonly scopes = new WeakSet<ActionScope>();
	private readonly unboundInvocations = new WeakMap<
		UnboundActionInvocation,
		ActionInvocationBinding
	>();
	private disposed = false;
	private nextExecutionOrdinal = 1;
	private nextScopeOrdinal = 1;

	public constructor(options: ActionRuntimeOptions = {}) {
		this.options = options;
		this.createExecutionId =
			options.createExecutionId ??
			(() => {
				const id = `action-execution-${this.nextExecutionOrdinal}`;
				this.nextExecutionOrdinal += 1;
				return id;
			});
	}

	public createTypedRuntime<Catalog>(): TypedActionRuntime<Catalog> {
		const catalogToken = Object.freeze({});
		return {
			execute: <Id extends ActionCatalogId<Catalog>>(
				actionId: Id,
				input: ActionInputFor<Catalog, Id>,
				invocation: ActionInvocation,
			) =>
				this.executeForCatalog<ActionOutputFor<Catalog, Id>>(
					catalogToken,
					actionId,
					input,
					invocation,
				),
			register: <Id extends ActionCatalogId<Catalog>>(
				scope: ActionScope,
				definition: CatalogActionDefinition<Catalog, Id>,
			) => this.registerInternal(catalogToken, scope, definition),
			resolveAction: <Id extends ActionCatalogId<Catalog>>(
				actionId: Id,
				input: ActionInputFor<Catalog, Id>,
				invocation: ActionInvocation,
			) => this.resolveActionForCatalog(catalogToken, actionId, input, invocation),
		};
	}

	public createScope(id: string, parent: ActionScope | null = null): ActionScope {
		this.assertActive();
		if (id.length === 0) {
			throw new Error('Action scope id must not be empty');
		}
		if (parent !== null && !this.scopes.has(parent)) {
			throw new Error(`Action scope parent "${parent.id}" belongs to another runtime`);
		}
		const scope = Object.freeze({ id, parent });
		this.scopes.add(scope);
		this.scopeOrdinals.set(scope, this.nextScopeOrdinal);
		this.nextScopeOrdinal += 1;
		return scope;
	}

	public register<Input, Output>(
		scope: ActionScope,
		definition: ActionDefinition<Input, Output>,
	): ActionDisposable {
		return this.registerInternal(null, scope, definition);
	}

	private registerInternal<Input, Output>(
		catalogToken: object | null,
		scope: ActionScope,
		definition: ActionDefinition<Input, Output>,
	): ActionDisposable {
		this.assertActive();
		this.assertOwnedScope(scope);
		const scopeRegistrations = this.registrations.get(scope) ?? new Map<string, StoredAction>();
		if (scopeRegistrations.has(definition.id)) {
			throw new Error(`Action "${definition.id}" is already registered in scope "${scope.id}"`);
		}
		const stored: StoredAction = {
			automation: definition.automation ?? 'denied',
			catalogToken,
			concurrency: definition.execution.concurrency,
			descriptor: definition,
			getDiscoveryAvailability: (context) => definition.getDiscoveryAvailability?.(context) ?? null,
			inputName: definition.input.name,
			parse: (input) => {
				const parsed = definition.input.parse(input);
				if (!parsed.success) {
					return parsed;
				}
				const value = parsed.data;
				return {
					success: true,
					data: {
						concurrencyKey: (context) => definition.execution.concurrencyKey(value, context),
						execute: (context) => definition.handler.execute(context, value),
						getAvailability: (context) => definition.getAvailability(context, value),
						getConfirmation: (context) => definition.confirmation?.(value, context) ?? null,
					},
				};
			},
		};
		scopeRegistrations.set(definition.id, stored);
		this.registrations.set(scope, scopeRegistrations);
		this.emitChanged();

		let active = true;
		return {
			dispose: () => {
				if (!active) {
					return;
				}
				active = false;
				const registrations = this.registrations.get(scope);
				registrations?.delete(definition.id);
				if (registrations?.size === 0) {
					this.registrations.delete(scope);
				}
				this.emitChanged();
			},
		};
	}

	public registerConfirmationPresenter(
		scope: ActionScope,
		presenter: ActionConfirmationPresenter,
	): ActionDisposable {
		this.assertActive();
		this.assertOwnedScope(scope);
		if (this.confirmationPresenters.has(scope)) {
			throw new Error(`A confirmation presenter is already registered in scope "${scope.id}"`);
		}
		this.confirmationPresenters.set(scope, presenter);
		let active = true;
		return {
			dispose: () => {
				if (!active) {
					return;
				}
				active = false;
				if (this.confirmationPresenters.get(scope) === presenter) {
					this.confirmationPresenters.delete(scope);
				}
			},
		};
	}

	public list(
		invocation: ActionInvocation,
		inputByActionId: Readonly<Record<string, unknown>> = {},
	): readonly ResolvedAction[] {
		return this.discover(invocation, { inputByActionId })
			.filter(
				(result): result is Extract<ActionDiscoveryResult, { readonly kind: 'ready' }> =>
					result.kind === 'ready',
			)
			.map(({ availability, descriptor, scope }) => ({
				availability,
				descriptor,
				scope,
			}));
	}

	public discover(
		invocation: ActionInvocation,
		options: ActionDiscoveryOptions = {},
	): readonly ActionDiscoveryResult[] {
		this.assertOwnedScope(invocation.scope);
		const discovered: ActionDiscoveryResult[] = [];
		const seen = new Set<string>();
		const includeDescendants = options.includeDescendants === true;
		const lineage = new Set(this.discoveryScopes(invocation.scope, false));
		for (const scope of this.discoveryScopes(invocation.scope, includeDescendants)) {
			for (const [actionId, stored] of this.registrations.get(scope) ?? []) {
				const resolvesFromInvocation = lineage.has(scope);
				if (resolvesFromInvocation && seen.has(actionId)) {
					continue;
				}
				if (resolvesFromInvocation) {
					seen.add(actionId);
				}
				if (invocation.source === 'automation' && stored.automation !== 'allowed') {
					continue;
				}
				const boundInvocation = this.snapshotInvocation(invocation, scope);
				const input = options.inputByActionId?.[actionId];
				const parsed = this.parseForResolution(stored, input);
				if (parsed === null) {
					const availability = this.readDiscoveryAvailabilityForResolution(stored, boundInvocation);
					if (availability === null || availability.kind === 'hidden') {
						continue;
					}
					discovered.push({
						availability,
						descriptor: stored.descriptor,
						invocation: this.createUnboundInvocation(actionId, stored, scope, boundInvocation),
						kind: 'requires-input',
						scope,
					});
					continue;
				}
				const availability = this.readAvailabilityForResolution(stored, boundInvocation, parsed);
				if (availability === null || availability.kind === 'hidden') {
					continue;
				}
				discovered.push({
					availability,
					descriptor: stored.descriptor,
					invocation: this.createPreparedInvocation(
						actionId,
						stored,
						scope,
						parsed,
						boundInvocation,
					),
					kind: 'ready',
					scope,
				});
			}
		}
		return discovered;
	}

	public prepareInvocation(
		unbound: UnboundActionInvocation,
		input: unknown,
	): Extract<ActionDiscoveryResult, { readonly kind: 'ready' }> | null {
		if (this.disposed) {
			return null;
		}
		const binding = this.unboundInvocations.get(unbound);
		if (binding === undefined || !this.isInvocationBindingActive(binding)) {
			return null;
		}
		const parsed = this.parseForResolution(binding.action, input);
		if (parsed === null) {
			return null;
		}
		const availability = this.readAvailabilityForResolution(
			binding.action,
			binding.invocation,
			parsed,
		);
		if (availability === null || availability.kind === 'hidden') {
			return null;
		}
		return {
			availability,
			descriptor: binding.action.descriptor,
			invocation: this.createPreparedInvocation(
				binding.actionId,
				binding.action,
				binding.scope,
				parsed,
				binding.invocation,
			),
			kind: 'ready',
			scope: binding.scope,
		};
	}

	public resolveAction(
		actionId: string,
		input: unknown,
		invocation: ActionInvocation,
	): ResolvedAction | null {
		this.assertOwnedScope(invocation.scope);
		const resolved = this.resolve(actionId, invocation.scope);
		if (
			resolved === null ||
			(invocation.source === 'automation' && resolved.action.automation !== 'allowed')
		) {
			return null;
		}
		const parsed = this.parseForResolution(resolved.action, input);
		if (parsed === null) {
			return null;
		}
		const availability = this.readAvailabilityForResolution(resolved.action, invocation, parsed);
		if (availability === null || availability.kind === 'hidden') {
			return null;
		}
		return {
			availability,
			descriptor: resolved.action.descriptor,
			scope: resolved.scope,
		};
	}

	private resolveActionForCatalog(
		catalogToken: object,
		actionId: string,
		input: unknown,
		invocation: ActionInvocation,
	): ResolvedAction | null {
		this.assertOwnedScope(invocation.scope);
		const resolved = this.resolve(actionId, invocation.scope);
		if (resolved?.action.catalogToken !== catalogToken) {
			return null;
		}
		return this.resolveAction(actionId, input, invocation);
	}

	private executeForCatalog<Output>(
		catalogToken: object,
		actionId: string,
		input: unknown,
		invocation: ActionInvocation,
	): ActionExecution<Output> {
		if (this.disposed) {
			return this.executeInternal<Output>(actionId, input, invocation);
		}
		this.assertOwnedScope(invocation.scope);
		const resolved = this.resolve(actionId, invocation.scope);
		if (resolved?.action.catalogToken !== catalogToken) {
			const execution = new ManagedActionExecution<Output>(this.createExecutionId());
			execution.settle({ kind: 'blocked', code: 'catalog-registration-mismatch' });
			return execution;
		}
		return this.executeInternal<Output>(actionId, input, invocation);
	}

	public execute(actionId: string, input: unknown, invocation: ActionInvocation): ActionExecution {
		return this.executeInternal<unknown>(actionId, input, invocation);
	}

	private executeInternal<Output = unknown>(
		actionId: string,
		input: unknown,
		invocation: ActionInvocation,
	): ActionExecution<Output>;
	private executeInternal(
		actionId: string,
		input: unknown,
		invocation: ActionInvocation,
	): ActionExecution {
		const invocationScope = invocation.scope;
		this.assertOwnedScope(invocationScope);
		const execution = this.createManagedExecution();
		if (this.disposed) {
			execution.settle({ kind: 'cancelled' });
			return execution;
		}
		const boundInvocation = this.snapshotInvocation(invocation, invocationScope);
		const resolved = this.resolve(actionId, invocationScope);
		if (
			resolved === null ||
			(boundInvocation.source === 'automation' && resolved.action.automation !== 'allowed')
		) {
			execution.settle({ kind: 'unsupported' });
			return execution;
		}
		const parsed = this.parseInput(resolved.action, input, execution);
		if (parsed === null) {
			return execution;
		}
		return this.executeResolved(
			actionId,
			resolved.action,
			resolved.scope,
			parsed,
			boundInvocation,
			execution,
		);
	}

	public executePrepared(prepared: PreparedActionInvocation): ActionExecution {
		const execution = this.createManagedExecution();
		if (this.disposed) {
			execution.settle({ kind: 'cancelled' });
			return execution;
		}
		const binding = this.preparedInvocations.get(prepared);
		if (binding === undefined) {
			execution.settle({ kind: 'blocked', code: 'invalid-prepared-invocation' });
			return execution;
		}
		if (!this.isInvocationBindingActive(binding)) {
			execution.settle({ kind: 'blocked', code: 'stale-prepared-invocation' });
			return execution;
		}
		if (binding.invocation.source === 'automation' && binding.action.automation !== 'allowed') {
			execution.settle({ kind: 'unsupported' });
			return execution;
		}
		return this.executeResolved(
			binding.actionId,
			binding.action,
			binding.scope,
			binding.input,
			binding.invocation,
			execution,
		);
	}

	private executeResolved(
		actionId: string,
		action: StoredAction,
		scope: ActionScope,
		parsed: ParsedActionInput,
		invocation: ActionInvocation,
		execution: ManagedActionExecution<unknown>,
	): ActionExecution {
		const availability = this.readAvailability(action, invocation, parsed, execution);
		if (availability === null || !this.acceptAvailability(availability, execution)) {
			return execution;
		}

		const policy = action.concurrency;
		let semanticConcurrencyKey: string;
		try {
			semanticConcurrencyKey = parsed.concurrencyKey(invocation);
		} catch (error) {
			this.failUnexpected(action.descriptor.id, error, execution);
			return execution;
		}
		const concurrencyKey = `${this.scopeOrdinal(scope)}:${actionId}:${semanticConcurrencyKey}`;
		const start = () => {
			if (execution.isTerminal()) {
				this.finishConcurrency(concurrencyKey, execution, policy);
				return;
			}
			this.begin(execution, action, parsed, invocation, () => {
				this.finishConcurrency(concurrencyKey, execution, policy);
			});
		};
		void execution.completion.then(() => {
			this.finishConcurrency(concurrencyKey, execution, policy);
		});
		if (!this.schedule(concurrencyKey, policy, execution, start)) {
			return execution;
		}
		start();
		return execution;
	}

	public notifyChanged(): void {
		if (this.disposed) return;
		this.emitChanged();
	}

	public subscribe(listener: () => void): ActionDisposable {
		this.assertActive();
		this.listeners.add(listener);
		return {
			dispose: () => {
				this.listeners.delete(listener);
			},
		};
	}

	public dispose(): void {
		if (this.disposed) {
			return;
		}
		this.disposed = true;
		this.registrations.clear();
		this.confirmationPresenters.clear();
		this.concurrencyEntries.clear();
		for (const execution of this.executions) {
			execution.cancel();
		}
		this.listeners.clear();
	}

	private acceptAvailability(
		availability: ActionAvailability,
		execution: ManagedActionExecution<unknown>,
	): boolean {
		if (availability.kind === 'enabled') {
			return true;
		}
		execution.settle(
			availability.kind === 'hidden'
				? { kind: 'unsupported' }
				: { kind: 'blocked', code: 'action-unavailable', details: availability.reason },
		);
		return false;
	}

	private assertActive(): void {
		if (this.disposed) {
			throw new Error('Action runtime is disposed');
		}
	}

	private assertOwnedScope(scope: ActionScope): void {
		if (!this.scopes.has(scope)) {
			throw new Error(`Action scope "${scope.id}" belongs to another runtime`);
		}
	}

	private begin(
		execution: ManagedActionExecution<unknown>,
		action: StoredAction,
		input: ParsedActionInput,
		invocation: ActionInvocation,
		onFinished: () => void,
	): void {
		let confirmation: ActionConfirmation | null;
		try {
			confirmation = input.getConfirmation(invocation);
		} catch (error) {
			this.failUnexpected(action.descriptor.id, error, execution);
			onFinished();
			return;
		}
		if (confirmation === null) {
			this.runHandler(execution, action, input, invocation, onFinished);
			return;
		}
		const confirm = this.resolveConfirmationPresenter(invocation.scope) ?? this.options.confirm;
		if (confirm === undefined) {
			execution.settle({ kind: 'blocked', code: 'confirmation-unavailable' });
			onFinished();
			return;
		}
		execution.setState('awaiting-confirmation');
		if (execution.isTerminal() || execution.signal.aborted) {
			execution.settle({ kind: 'cancelled' });
			onFinished();
			return;
		}
		let accepted: boolean | Promise<boolean>;
		let stopWatching: (() => void) | undefined;
		try {
			stopWatching = confirmation.watch?.(() => execution.cancel());
			accepted = confirm(confirmation, invocation, execution.signal);
		} catch (error) {
			stopWatching?.();
			this.failUnexpected(action.descriptor.id, error, execution);
			onFinished();
			return;
		}
		try {
			if (isPromiseLike(accepted)) {
				void Promise.resolve(accepted).then(
					(value) => {
						stopWatching?.();
						this.afterConfirmation(value, execution, action, input, invocation, onFinished);
					},
					(error: unknown) => {
						stopWatching?.();
						this.failUnexpected(action.descriptor.id, error, execution);
						onFinished();
					},
				);
				return;
			}
			stopWatching?.();
			this.afterConfirmation(accepted, execution, action, input, invocation, onFinished);
		} catch (error) {
			this.failUnexpected(action.descriptor.id, error, execution);
			onFinished();
		}
	}

	private afterConfirmation(
		accepted: boolean,
		execution: ManagedActionExecution<unknown>,
		action: StoredAction,
		input: ParsedActionInput,
		invocation: ActionInvocation,
		onFinished: () => void,
	): void {
		if (execution.isTerminal()) {
			onFinished();
			return;
		}
		if (!accepted || execution.signal.aborted) {
			execution.settle({ kind: 'cancelled' });
			onFinished();
			return;
		}
		const availability = this.readAvailability(action, invocation, input, execution);
		if (availability === null || !this.acceptAvailability(availability, execution)) {
			onFinished();
			return;
		}
		this.runHandler(execution, action, input, invocation, onFinished);
	}

	private emitChanged(): void {
		for (const listener of new Set(this.listeners)) {
			if (!this.listeners.has(listener)) continue;
			try {
				listener();
			} catch {
				// Registry observers cannot take ownership of registration lifecycle.
				continue;
			}
		}
	}

	private discoveryScopes(startScope: ActionScope, includeDescendants: boolean): ActionScope[] {
		const lineage: ActionScope[] = [];
		for (let scope: ActionScope | null = startScope; scope !== null; scope = scope.parent) {
			lineage.push(scope);
		}
		if (!includeDescendants) {
			return lineage;
		}
		const descendants = [...this.registrations.keys()]
			.filter((scope) => !lineage.includes(scope) && this.isDescendantOf(scope, startScope))
			.sort((left, right) => this.scopeOrdinal(left) - this.scopeOrdinal(right));
		return [...lineage, ...descendants];
	}

	private createManagedExecution(): ManagedActionExecution<unknown> {
		const execution = new ManagedActionExecution<unknown>(this.createExecutionId());
		this.executions.add(execution);
		void execution.completion.then(() => {
			this.executions.delete(execution);
		});
		return execution;
	}

	private createPreparedInvocation(
		actionId: string,
		action: StoredAction,
		scope: ActionScope,
		input: ParsedActionInput,
		invocation: ActionInvocation,
	): PreparedActionInvocation {
		const prepared = Object.freeze({ actionId, invocation });
		this.preparedInvocations.set(prepared, {
			action,
			actionId,
			input,
			invocation,
			scope,
		});
		return prepared;
	}

	private createUnboundInvocation(
		actionId: string,
		action: StoredAction,
		scope: ActionScope,
		invocation: ActionInvocation,
	): UnboundActionInvocation {
		const unbound = Object.freeze({
			actionId,
			inputName: action.inputName,
			invocation,
		});
		this.unboundInvocations.set(unbound, {
			action,
			actionId,
			invocation,
			scope,
		});
		return unbound;
	}

	private failUnexpected(
		actionId: string,
		error: unknown,
		execution: ManagedActionExecution<unknown>,
	): void {
		this.reportUnexpected(actionId, error);
		execution.settle({ kind: 'failed', failure: { code: 'unexpected-error' } });
	}

	private finishConcurrency(
		key: string,
		execution: ManagedActionExecution<unknown>,
		policy: ActionConcurrency,
	): void {
		if (policy === 'parallel') {
			return;
		}
		const entry = this.concurrencyEntries.get(key);
		if (entry === undefined || entry.active !== execution) {
			return;
		}
		entry.active = null;
		this.drainConcurrencyEntry(key, entry);
	}

	private drainConcurrencyEntry(key: string, entry: ConcurrencyEntry): void {
		if (entry.draining) {
			return;
		}
		entry.draining = true;
		try {
			while (entry.active === null) {
				const next = entry.queue.shift();
				if (next === undefined) {
					if (this.concurrencyEntries.get(key) === entry) {
						this.concurrencyEntries.delete(key);
					}
					return;
				}
				next();
			}
		} finally {
			entry.draining = false;
		}
	}

	private parseInput(
		action: StoredAction,
		input: unknown,
		execution: ManagedActionExecution<unknown>,
	): ParsedActionInput | null {
		try {
			const parsed = action.parse(input);
			if (parsed.success) {
				return parsed.data;
			}
			execution.settle({
				kind: 'blocked',
				code: 'invalid-action-input',
				details: { validationCode: parsed.code },
			});
			return null;
		} catch (error) {
			this.failUnexpected(action.descriptor.id, error, execution);
			return null;
		}
	}

	private parseForResolution(action: StoredAction, input: unknown): ParsedActionInput | null {
		try {
			const parsed = action.parse(input);
			return parsed.success ? parsed.data : null;
		} catch (error) {
			this.reportUnexpected(action.descriptor.id, error);
			return null;
		}
	}

	private readAvailability(
		action: StoredAction,
		context: ActionAvailabilityContext,
		input: ParsedActionInput,
		execution: ManagedActionExecution<unknown>,
	): ActionAvailability | null {
		try {
			return input.getAvailability(context);
		} catch (error) {
			this.failUnexpected(action.descriptor.id, error, execution);
			return null;
		}
	}

	private readAvailabilityForResolution(
		action: StoredAction,
		context: ActionAvailabilityContext,
		input: ParsedActionInput,
	): ActionAvailability | null {
		try {
			return input.getAvailability(context);
		} catch (error) {
			this.reportUnexpected(action.descriptor.id, error);
			return null;
		}
	}

	private readDiscoveryAvailabilityForResolution(
		action: StoredAction,
		context: ActionAvailabilityContext,
	): ActionAvailability | null {
		try {
			return action.getDiscoveryAvailability(context);
		} catch (error) {
			this.reportUnexpected(action.descriptor.id, error);
			return null;
		}
	}

	private reportUnexpected(actionId: string, error: unknown): void {
		try {
			this.options.onUnexpectedError?.(actionId, error);
		} catch {
			// A diagnostic sink never owns or alters the Action execution lifecycle.
			return;
		}
	}

	private isDescendantOf(scope: ActionScope, ancestor: ActionScope): boolean {
		for (let candidate = scope.parent; candidate !== null; candidate = candidate.parent) {
			if (candidate === ancestor) {
				return true;
			}
		}
		return false;
	}

	private isInvocationBindingActive(binding: ActionInvocationBinding): boolean {
		return this.registrations.get(binding.scope)?.get(binding.actionId) === binding.action;
	}

	private resolve(
		actionId: string,
		startScope: ActionScope,
	): { readonly action: StoredAction; readonly scope: ActionScope } | null {
		for (let scope: ActionScope | null = startScope; scope !== null; scope = scope.parent) {
			const action = this.registrations.get(scope)?.get(actionId);
			if (action !== undefined) {
				return { action, scope };
			}
		}
		return null;
	}

	private resolveConfirmationPresenter(
		startScope: ActionScope,
	): ActionConfirmationPresenter | null {
		for (let scope: ActionScope | null = startScope; scope !== null; scope = scope.parent) {
			const presenter = this.confirmationPresenters.get(scope);
			if (presenter !== undefined) {
				return presenter;
			}
		}
		return null;
	}

	private runHandler(
		execution: ManagedActionExecution<unknown>,
		action: StoredAction,
		input: ParsedActionInput,
		invocation: ActionInvocation,
		onFinished: () => void,
	): void {
		if (execution.signal.aborted) {
			execution.settle({ kind: 'cancelled' });
			onFinished();
			return;
		}
		const availability = this.readAvailability(action, invocation, input, execution);
		if (availability === null || !this.acceptAvailability(availability, execution)) {
			onFinished();
			return;
		}
		execution.setState('running');
		if (execution.isTerminal() || execution.signal.aborted) {
			execution.settle({ kind: 'cancelled' });
			onFinished();
			return;
		}
		execution.markHandlerStarted();
		const context: ActionExecutionContext = {
			...invocation,
			executionId: execution.executionId,
			signal: execution.signal,
			markCommitted: () => execution.markCommitted(),
			reportProgress: (progress) => execution.reportProgress(progress),
		};
		let result: ActionResult<unknown> | Promise<ActionResult<unknown>>;
		try {
			result = input.execute(context);
		} catch (error) {
			this.failUnexpected(action.descriptor.id, error, execution);
			onFinished();
			return;
		}
		try {
			if (isPromiseLike(result)) {
				void Promise.resolve(result).then(
					(value) => {
						this.settleHandlerResult(action.descriptor.id, execution, value);
						onFinished();
					},
					(error: unknown) => {
						this.settleHandlerFailure(action.descriptor.id, execution, error);
						onFinished();
					},
				);
				return;
			}
			this.settleHandlerResult(action.descriptor.id, execution, result);
			onFinished();
		} catch (error) {
			this.failUnexpected(action.descriptor.id, error, execution);
			onFinished();
		}
	}

	private schedule(
		key: string,
		policy: ActionConcurrency,
		execution: ManagedActionExecution<unknown>,
		start: () => void,
	): boolean {
		if (policy === 'parallel') {
			return true;
		}
		const entry = this.concurrencyEntries.get(key);
		if (entry === undefined) {
			this.concurrencyEntries.set(key, { active: execution, draining: false, queue: [] });
			return true;
		}
		if (policy === 'reject-duplicate') {
			execution.settle({ kind: 'blocked', code: 'duplicate-action-execution' });
			return false;
		}
		if (policy === 'replace-latest') {
			const previous = entry.active;
			// Cancellation can synchronously schedule an even newer invocation.
			entry.active = execution;
			previous?.cancel();
			return true;
		}
		entry.queue.push(() => {
			entry.active = execution;
			start();
		});
		return false;
	}

	private scopeOrdinal(scope: ActionScope): number {
		const ordinal = this.scopeOrdinals.get(scope);
		if (ordinal === undefined) {
			throw new Error(`Action scope "${scope.id}" belongs to another runtime`);
		}
		return ordinal;
	}

	private settleHandlerResult(
		actionId: string,
		execution: ManagedActionExecution<unknown>,
		result: ActionResult<unknown>,
	): void {
		if (execution.isCommitted() && result.kind === 'cancelled') {
			this.failUnexpected(
				actionId,
				new Error('An Action handler returned cancelled after crossing its commit point'),
				execution,
			);
			return;
		}
		if (execution.signal.aborted && !execution.isCommitted() && result.kind !== 'cancelled') {
			execution.settle({ kind: 'cancelled' });
			return;
		}
		execution.settle(result);
	}

	private settleHandlerFailure(
		actionId: string,
		execution: ManagedActionExecution<unknown>,
		error: unknown,
	): void {
		if (execution.signal.aborted && !execution.isCommitted()) {
			execution.settle({ kind: 'cancelled' });
			return;
		}
		this.failUnexpected(actionId, error, execution);
	}

	private snapshotInvocation(invocation: ActionInvocation, scope: ActionScope): ActionInvocation {
		return Object.freeze({
			scope,
			source: invocation.source,
			...(invocation.capabilities === undefined
				? {}
				: { capabilities: Object.freeze({ ...invocation.capabilities }) }),
			...(invocation.focus === undefined ? {} : { focus: Object.freeze({ ...invocation.focus }) }),
			...(invocation.target === undefined
				? {}
				: { target: Object.freeze({ ...invocation.target }) }),
		});
	}
}
