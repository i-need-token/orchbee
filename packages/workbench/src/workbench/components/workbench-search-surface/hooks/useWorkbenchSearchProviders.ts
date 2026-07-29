import { resolveOrchbeeLocale } from '@orchbee/i18n';
import * as React from 'react';

import type { WorkbenchSearchProvider } from '../../../types/workbench-search-provider';
import { WorkbenchSearchProviderCoordinator } from '../model/WorkbenchSearchProviderCoordinator';
import type {
	WorkbenchSearchProviderRun,
	WorkbenchSearchProviderState,
} from '../model/workbench-search-provider-state';
import type { WorkbenchSearchState } from '../model/workbench-search-state';

interface UseWorkbenchSearchProvidersOptions {
	readonly locale: string;
	readonly providers: readonly WorkbenchSearchProvider[];
	readonly state: Exclude<WorkbenchSearchState, { readonly kind: 'closed' }>;
}

interface WorkbenchSearchProvidersView {
	readonly providersById: ReadonlyMap<string, WorkbenchSearchProvider>;
	readonly retry: (providerId: string) => void;
	readonly runs: readonly WorkbenchSearchProviderRun[];
}

export function useWorkbenchSearchProviders({
	locale,
	providers,
	state,
}: UseWorkbenchSearchProvidersOptions): WorkbenchSearchProvidersView {
	const resolvedLocale = resolveOrchbeeLocale([locale]);
	const coordinator = React.useMemo(
		() => new WorkbenchSearchProviderCoordinator(providers, resolvedLocale),
		[providers, resolvedLocale],
	);
	const request = state.kind === 'browsing' ? state : state.parent;
	const providerState = React.useSyncExternalStore<WorkbenchSearchProviderState>(
		coordinator.subscribe,
		coordinator.getSnapshot,
		coordinator.getSnapshot,
	);

	React.useEffect(() => {
		const connection = coordinator.connect();
		coordinator.request({
			filter: request.filter,
			query: request.query,
			queryRevision: request.queryRevision,
			sessionId: state.sessionId,
		});
		return () => connection.dispose();
	}, [coordinator, request.filter, request.query, request.queryRevision, state.sessionId]);

	return React.useMemo(
		() => ({
			providersById: new Map(providers.map((provider) => [provider.id, provider])),
			retry: (providerId: string) => coordinator.retry(providerId),
			runs: providerState.runs,
		}),
		[coordinator, providerState.runs, providers],
	);
}
