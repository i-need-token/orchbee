export type ActionExecutionId = string;
type ActionScopeId = string;

export interface ActionDisposable {
	dispose(): void;
}

export interface ActionMessageDescriptor {
	readonly defaultMessage?: string | undefined;
	readonly description?: string | object | undefined;
	readonly id?: string | undefined;
	readonly values?: Readonly<Record<string, boolean | number | string>> | undefined;
}

interface ActionCategory {
	readonly id: string;
	readonly title: ActionMessageDescriptor;
}

export interface ActionDescriptor {
	readonly category: ActionCategory;
	readonly description?: ActionMessageDescriptor | undefined;
	readonly icon?: string | undefined;
	readonly id: string;
	readonly keywords?: readonly ActionMessageDescriptor[] | undefined;
	readonly title: ActionMessageDescriptor;
}

export interface ActionScope {
	readonly id: ActionScopeId;
	readonly parent: ActionScope | null;
}

export interface ActionTarget {
	readonly id: string;
	readonly type: string;
}

export type ActionInvocationSource =
	| 'automation'
	| 'button'
	| 'command-surface'
	| 'context-menu'
	| 'keyboard'
	| 'menu'
	| 'pointer'
	| 'programmatic';

export interface ActionInvocation {
	readonly capabilities?: Readonly<Record<string, unknown>> | undefined;
	readonly focus?: Readonly<Record<string, string | boolean | null>> | undefined;
	readonly scope: ActionScope;
	readonly source: ActionInvocationSource;
	readonly target?: ActionTarget | undefined;
}

export interface PreparedActionInvocation {
	readonly actionId: string;
	readonly invocation: ActionInvocation;
}

export interface UnboundActionInvocation {
	readonly actionId: string;
	readonly inputName: string;
	readonly invocation: ActionInvocation;
}

export type ActionAvailabilityContext = ActionInvocation;

export type ActionAvailability =
	| { readonly kind: 'enabled'; readonly checked?: boolean | undefined }
	| {
			readonly kind: 'disabled';
			readonly checked?: boolean | undefined;
			readonly reason: ActionMessageDescriptor;
	  }
	| { readonly kind: 'hidden' };

export type ActionValidationResult<Input> =
	| { readonly success: true; readonly data: Input }
	| { readonly success: false; readonly code: string };

export interface ActionInputValidation<Input> {
	readonly name: string;
	parse(input: unknown): ActionValidationResult<Input>;
}

export type ActionResult<Output = unknown> =
	| { readonly kind: 'completed'; readonly value?: Output | undefined }
	| { readonly kind: 'unchanged'; readonly value?: Output | undefined }
	| { readonly kind: 'blocked'; readonly code: string; readonly details?: unknown }
	| { readonly kind: 'cancelled' }
	| { readonly kind: 'failed'; readonly failure: ActionFailure }
	| { readonly kind: 'unsupported' };

interface ActionFailure {
	readonly code: string;
	readonly details?: unknown;
}

export interface ActionConfirmation {
	readonly cancelLabel?: ActionMessageDescriptor | undefined;
	readonly confirmLabel: ActionMessageDescriptor;
	/**
	 * The answer that holds the initial focus, so unmodified Enter gives it. A close confirmation
	 * answers `confirm`, so Enter closes in every Client; omitted, the cancel answer holds it.
	 */
	readonly defaultAnswer?: 'cancel' | 'confirm' | undefined;
	readonly description: ActionMessageDescriptor;
	readonly risk: 'destructive' | 'elevated' | 'routine';
	readonly title: ActionMessageDescriptor;
	/**
	 * Withdraws the open question once what it asks about no longer holds. Runs while the question is
	 * open and returns how to stop.
	 */
	readonly watch?: ((withdraw: () => void) => () => void) | undefined;
}

export type ActionConfirmationPresenter = (
	confirmation: ActionConfirmation,
	invocation: ActionInvocation,
	signal: AbortSignal,
) => boolean | Promise<boolean>;

export interface ActionProgress {
	readonly phase: string;
	readonly completed?: number | undefined;
	readonly total?: number | undefined;
	readonly message?: ActionMessageDescriptor | undefined;
}

export type ActionConcurrency = 'parallel' | 'reject-duplicate' | 'replace-latest' | 'serialize';
type ActionExecutionMode = 'background' | 'external-job' | 'foreground';

interface ActionExecutionPolicy<Input> {
	readonly concurrency: ActionConcurrency;
	readonly concurrencyKey: (input: Input, context: ActionAvailabilityContext) => string;
	readonly mode: ActionExecutionMode;
}

export interface ActionExecutionContext extends ActionInvocation {
	readonly executionId: ActionExecutionId;
	readonly signal: AbortSignal;
	markCommitted(): void;
	reportProgress(progress: ActionProgress): void;
}

interface ActionHandler<Input, Output> {
	execute(
		context: ActionExecutionContext,
		input: Input,
	): ActionResult<Output> | Promise<ActionResult<Output>>;
}

export interface ActionDefinition<Input, Output> extends ActionDescriptor {
	readonly automation?: 'allowed' | 'denied' | undefined;
	readonly confirmation?:
		| ((input: Input, context: ActionAvailabilityContext) => ActionConfirmation | null)
		| undefined;
	readonly execution: ActionExecutionPolicy<Input>;
	readonly getAvailability: (
		context: ActionAvailabilityContext,
		input: Input,
	) => ActionAvailability;
	readonly getDiscoveryAvailability?:
		| ((context: ActionAvailabilityContext) => ActionAvailability)
		| undefined;
	readonly handler: ActionHandler<Input, Output>;
	readonly input: ActionInputValidation<Input>;
}

type ActionExecutionState =
	| 'awaiting-confirmation'
	| 'blocked'
	| 'cancelled'
	| 'cancelling'
	| 'completed'
	| 'failed'
	| 'resolving'
	| 'running'
	| 'unchanged'
	| 'unsupported';

export interface ActionExecutionSnapshot<Output = unknown> {
	readonly progress: ActionProgress | null;
	readonly result: ActionResult<Output> | null;
	readonly state: ActionExecutionState;
}

export interface ActionExecution<Output = unknown> {
	readonly completion: Promise<ActionResult<Output>>;
	readonly executionId: ActionExecutionId;
	cancel(): void;
	getSnapshot(): ActionExecutionSnapshot<Output>;
	subscribe(listener: () => void): ActionDisposable;
}

export interface ResolvedAction {
	readonly availability: ActionAvailability;
	readonly descriptor: ActionDescriptor;
	readonly scope: ActionScope;
}

export type ActionDiscoveryResult =
	| {
			readonly availability: ActionAvailability;
			readonly descriptor: ActionDescriptor;
			readonly invocation: PreparedActionInvocation;
			readonly kind: 'ready';
			readonly scope: ActionScope;
	  }
	| {
			readonly availability: ActionAvailability;
			readonly descriptor: ActionDescriptor;
			readonly invocation: UnboundActionInvocation;
			readonly kind: 'requires-input';
			readonly scope: ActionScope;
	  };

export interface ActionDiscoveryOptions {
	readonly includeDescendants?: boolean | undefined;
	readonly inputByActionId?: Readonly<Record<string, unknown>> | undefined;
}

export interface ActionSignature<Input, Output = void> {
	readonly input: Input;
	readonly output: Output;
}

export type ActionCatalogId<Catalog> = Extract<keyof Catalog, string>;

export type ActionInputFor<Catalog, Id extends ActionCatalogId<Catalog>> =
	Catalog[Id] extends ActionSignature<infer Input, unknown> ? Input : never;

export type ActionOutputFor<Catalog, Id extends ActionCatalogId<Catalog>> =
	Catalog[Id] extends ActionSignature<unknown, infer Output> ? Output : never;

export type CatalogActionDefinition<
	Catalog,
	Id extends ActionCatalogId<Catalog> = ActionCatalogId<Catalog>,
> = {
	[CandidateId in Id]: ActionDefinition<
		ActionInputFor<Catalog, CandidateId>,
		ActionOutputFor<Catalog, CandidateId>
	> & {
		readonly id: CandidateId;
	};
}[Id];

export interface TypedActionRuntime<Catalog> {
	execute<Id extends ActionCatalogId<Catalog>>(
		actionId: Id,
		input: ActionInputFor<Catalog, Id>,
		invocation: ActionInvocation,
	): ActionExecution<ActionOutputFor<Catalog, Id>>;
	resolveAction<Id extends ActionCatalogId<Catalog>>(
		actionId: Id,
		input: ActionInputFor<Catalog, Id>,
		invocation: ActionInvocation,
	): ResolvedAction | null;
	register<Id extends ActionCatalogId<Catalog>>(
		scope: ActionScope,
		definition: CatalogActionDefinition<Catalog, Id>,
	): ActionDisposable;
}

export interface ActionRuntimeOptions {
	readonly confirm?: ActionConfirmationPresenter | undefined;
	readonly createExecutionId?: (() => ActionExecutionId) | undefined;
	readonly onUnexpectedError?: ((actionId: string, error: unknown) => void) | undefined;
}
