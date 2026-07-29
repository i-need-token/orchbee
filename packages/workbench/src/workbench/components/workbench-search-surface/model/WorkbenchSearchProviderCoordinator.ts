import type { ActionDisposable } from '@orchbee/actions';
import type { OrchbeeLocale } from '@orchbee/i18n';

import type {
	WorkbenchSearchProvider,
	WorkbenchSearchProviderResult,
	WorkbenchSearchProviderSection,
} from '../../../types/workbench-search-provider';
import type { WorkbenchSearchResult, WorkbenchSearchSection } from './workbench-search-result';
import {
	createWorkbenchSearchProviderState,
	transitionWorkbenchSearchProviderState,
	type WorkbenchSearchProviderEvent,
	type WorkbenchSearchProviderRequest,
	type WorkbenchSearchProviderRun,
	type WorkbenchSearchProviderState,
} from './workbench-search-provider-state';

interface ActiveProviderRequest {
	readonly attempt: number;
	readonly controller: AbortController;
}

export class WorkbenchSearchProviderCoordinator implements ActionDisposable {
	private readonly activeRequests = new Map<string, ActiveProviderRequest>();
	private readonly listeners = new Set<() => void>();
	private readonly locale: OrchbeeLocale;
	private providerDisposables: readonly ActionDisposable[] = [];
	private readonly providers: ReadonlyMap<string, WorkbenchSearchProvider>;
	private connected = false;
	private disposed = false;
	private state = createWorkbenchSearchProviderState();

	public constructor(providers: readonly WorkbenchSearchProvider[], locale: OrchbeeLocale) {
		this.locale = locale;
		const providersById = new Map<string, WorkbenchSearchProvider>();
		for (const provider of providers) {
			if (provider.id.length === 0) {
				throw new Error('Workbench search provider id must not be empty');
			}
			if (provider.label.length === 0) {
				throw new Error(`Workbench search provider "${provider.id}" label must not be empty`);
			}
			if (providersById.has(provider.id)) {
				throw new Error(`Workbench search provider "${provider.id}" is registered twice`);
			}
			if (!Number.isSafeInteger(provider.resultLimit) || provider.resultLimit <= 0) {
				throw new Error(
					`Workbench search provider "${provider.id}" must declare a positive integer resultLimit`,
				);
			}
			providersById.set(provider.id, provider);
		}
		this.providers = providersById;
	}

	public connect(): ActionDisposable {
		this.assertActive();
		if (this.connected) {
			throw new Error('Workbench search provider coordinator is already connected');
		}
		this.connected = true;
		this.providerDisposables = [...this.providers.values()].flatMap((provider) => {
			const disposable = provider.subscribe?.(() => {
				this.restartProvider(provider.id, 'provider-model-changed');
			});
			return disposable === undefined ? [] : [disposable];
		});
		return { dispose: this.disconnect };
	}

	public readonly getSnapshot = (): WorkbenchSearchProviderState => this.state;

	public request(request: WorkbenchSearchProviderRequest): void {
		this.assertActive();
		const providerIds = [...this.providers.values()]
			.filter((provider) => provider.filters.includes(request.filter))
			.map((provider) => provider.id);
		if (
			this.state.request?.sessionId === request.sessionId &&
			this.state.request.queryRevision === request.queryRevision &&
			this.state.request.filter === request.filter &&
			this.state.request.query === request.query &&
			providerIds.length === this.state.runs.length &&
			providerIds.every((providerId, index) => this.state.runs[index]?.providerId === providerId)
		) {
			return;
		}
		this.abortAll();
		this.apply({
			providerIds,
			request,
			type: 'request-changed',
		});
		for (const run of this.state.runs) {
			this.start(run);
		}
	}

	public retry(providerId: string): void {
		this.restartProvider(providerId, 'retry-requested');
	}

	public close(): void {
		if (this.disposed) {
			return;
		}
		this.abortAll();
		this.apply({ type: 'closed' });
	}

	public readonly subscribe = (listener: () => void): (() => void) => {
		this.assertActive();
		this.listeners.add(listener);
		return () => {
			this.listeners.delete(listener);
		};
	};

	public dispose(): void {
		if (this.disposed) {
			return;
		}
		this.disconnect();
		this.listeners.clear();
		this.disposed = true;
	}

	private readonly disconnect = (): void => {
		if (!this.connected) {
			this.close();
			return;
		}
		this.close();
		for (const disposable of this.providerDisposables) {
			disposable.dispose();
		}
		this.providerDisposables = [];
		this.connected = false;
	};

	private abortAll(): void {
		for (const request of this.activeRequests.values()) {
			request.controller.abort();
		}
		this.activeRequests.clear();
	}

	private abortProvider(providerId: string): void {
		this.activeRequests.get(providerId)?.controller.abort();
		this.activeRequests.delete(providerId);
	}

	private apply(event: WorkbenchSearchProviderEvent): void {
		const nextState = transitionWorkbenchSearchProviderState(this.state, event);
		if (nextState === this.state) {
			return;
		}
		this.state = nextState;
		for (const listener of this.listeners) {
			listener();
		}
	}

	private assertActive(): void {
		if (this.disposed) {
			throw new Error('Workbench search provider coordinator is disposed');
		}
	}

	private restartProvider(
		providerId: string,
		type: 'provider-model-changed' | 'retry-requested',
	): void {
		if (this.disposed) {
			return;
		}
		const previousRun = this.state.runs.find((run) => run.providerId === providerId);
		this.apply({ providerId, type });
		const nextRun = this.state.runs.find((run) => run.providerId === providerId);
		if (
			nextRun?.phase !== 'loading' ||
			previousRun === undefined ||
			nextRun.attempt === previousRun.attempt
		) {
			return;
		}
		this.abortProvider(providerId);
		this.start(nextRun);
	}

	private start(run: WorkbenchSearchProviderRun): void {
		const request = this.state.request;
		const provider = this.providers.get(run.providerId);
		if (request === null || provider === undefined || run.phase !== 'loading') {
			return;
		}
		const controller = new AbortController();
		const activeRequest = { attempt: run.attempt, controller };
		this.activeRequests.set(provider.id, activeRequest);
		const context = { filter: request.filter, locale: this.locale };
		const operation =
			request.query.trim().length === 0
				? () => provider.getDefaultSections(context, controller.signal)
				: () => provider.search(request.query, context, controller.signal);
		void Promise.resolve()
			.then(operation)
			.then(
				(sections) => {
					this.apply({
						attempt: run.attempt,
						providerId: provider.id,
						queryRevision: run.queryRevision,
						sections: normalizeProviderSections(provider, sections),
						sessionId: run.sessionId,
						type: 'provider-completed',
					});
				},
				() => {
					this.apply({
						attempt: run.attempt,
						failureCode: 'provider-failed',
						providerId: provider.id,
						queryRevision: run.queryRevision,
						sessionId: run.sessionId,
						type: 'provider-failed',
					});
				},
			)
			.finally(() => {
				if (this.activeRequests.get(provider.id) === activeRequest) {
					this.activeRequests.delete(provider.id);
				}
			});
	}
}

function normalizeProviderSections(
	provider: WorkbenchSearchProvider,
	sections: readonly WorkbenchSearchProviderSection[],
): readonly WorkbenchSearchSection[] {
	const normalizedSections: WorkbenchSearchSection[] = [];
	const sectionIndexes = new Map<string, number>();
	const resultIds = new Set<string>();
	let acceptedResultCount = 0;

	for (const section of sections) {
		if (section.id.length === 0 || acceptedResultCount >= provider.resultLimit) {
			continue;
		}
		let sectionIndex = sectionIndexes.get(section.id);
		if (sectionIndex === undefined) {
			sectionIndex = normalizedSections.length;
			sectionIndexes.set(section.id, sectionIndex);
			normalizedSections.push({
				id: `${provider.id}:${section.id}`,
				label: section.label,
				order: section.order,
				results: [],
			});
		}
		const normalizedSection = normalizedSections[sectionIndex];
		if (normalizedSection === undefined) {
			continue;
		}
		const results = [...normalizedSection.results];
		for (const result of section.results) {
			if (
				acceptedResultCount >= provider.resultLimit ||
				result.id.length === 0 ||
				result.availability.kind === 'hidden' ||
				resultIds.has(result.id)
			) {
				continue;
			}
			resultIds.add(result.id);
			results.push(normalizeProviderResult(provider, section, result));
			acceptedResultCount += 1;
		}
		normalizedSections[sectionIndex] = { ...normalizedSection, results };
	}

	return normalizedSections.filter((section) => section.results.length > 0);
}

function normalizeProviderResult(
	provider: WorkbenchSearchProvider,
	section: WorkbenchSearchProviderSection,
	result: WorkbenchSearchProviderResult,
): WorkbenchSearchResult {
	return {
		actionId:
			result.invocation.kind === 'ready'
				? result.invocation.prepared.actionId
				: result.invocation.unbound.actionId,
		availability: result.availability,
		canonicalLabel: result.canonicalLabel,
		...(result.description === undefined ? {} : { description: result.description }),
		groupId: `${provider.id}:${section.id}`,
		groupLabel: section.label,
		groupOrder: section.order,
		icon: result.icon,
		id: `${provider.id}:${result.id}`,
		invocation: result.invocation,
		keywords: [...result.keywords],
		label: result.label,
		providerId: provider.id,
		providerPriority: provider.priority,
		recommended: result.recommended ?? false,
		...(result.shortcut === undefined ? {} : { shortcut: result.shortcut }),
		...(result.surfaceDisabledReason === undefined
			? {}
			: { surfaceDisabledReason: result.surfaceDisabledReason }),
		visibility: result.visibility ?? 'always',
	};
}
