import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import type { IntlShape } from '@orchbee/i18n';
import * as React from 'react';

import type { WorkbenchShortcutPlatform } from '../../../types/workbench-shortcut-platform';
import { projectWorkbenchSearchCatalog } from '../model/project-workbench-search-catalog';
import type { WorkbenchSearchCatalog } from '../model/workbench-search-result';

interface UseWorkbenchSearchCatalogOptions {
	readonly intl: IntlShape;
	readonly platform: WorkbenchShortcutPlatform;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
}

export function useWorkbenchSearchCatalog({
	intl,
	platform,
	runtime,
	scope,
}: UseWorkbenchSearchCatalogOptions): WorkbenchSearchCatalog {
	const [runtimeRevision, incrementRuntimeRevision] = React.useReducer(
		(revision: number) => revision + 1,
		0,
	);

	React.useEffect(() => {
		const subscription = runtime.subscribe(incrementRuntimeRevision);
		return () => subscription.dispose();
	}, [runtime]);

	return React.useMemo(() => {
		void runtimeRevision;
		return projectWorkbenchSearchCatalog({ intl, platform, runtime, scope });
	}, [intl, platform, runtime, runtimeRevision, scope]);
}
