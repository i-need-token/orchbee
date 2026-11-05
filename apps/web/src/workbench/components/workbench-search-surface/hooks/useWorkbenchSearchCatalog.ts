import type { ActionRuntime, ActionScope } from '@orchbee/actions';
import type { IntlShape } from '@orchbee/i18n';
import * as React from 'react';

import { projectWorkbenchSearchCatalog } from '../model/project-workbench-search-catalog';
import type {
	WorkbenchSearchArgumentPicker,
	WorkbenchSearchCatalog,
} from '../model/workbench-search-result';

interface UseWorkbenchSearchCatalogOptions {
	readonly argumentPickerForAction: (actionId: string) => WorkbenchSearchArgumentPicker | undefined;
	readonly intl: IntlShape;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
	readonly shortcutLabelForAction: (actionId: string) => string | undefined;
}

export function useWorkbenchSearchCatalog({
	argumentPickerForAction,
	intl,
	runtime,
	scope,
	shortcutLabelForAction,
}: UseWorkbenchSearchCatalogOptions): WorkbenchSearchCatalog {
	const [runtimeRevision, incrementRuntimeRevision] = React.useReducer(
		(revision: number) => revision + 1,
		0,
	);

	React.useEffect(() => {
		const subscription = runtime.subscribe(incrementRuntimeRevision);
		// Registry changes may have occurred after render but before this subscription.
		incrementRuntimeRevision();
		return () => subscription.dispose();
	}, [runtime]);

	return React.useMemo(() => {
		void runtimeRevision;
		return projectWorkbenchSearchCatalog({
			argumentPickerForAction,
			intl,
			runtime,
			scope,
			shortcutLabelForAction,
		});
	}, [argumentPickerForAction, intl, runtime, runtimeRevision, scope, shortcutLabelForAction]);
}
