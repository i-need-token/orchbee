import type {
	ActionCatalogId,
	ActionAvailability,
	ActionExecution,
	ActionInputFor,
	ActionInvocationSource,
	ActionOutputFor,
	ActionScope,
} from '@orchbee/actions';

import type { TabCreateInput, TabStripActionCatalog } from '../actions/tab-strip-action-contracts';
import type { TabStripNewTabRequest, TabStripTabCreationIntent } from './tab-strip-options';

export interface TabStripActions {
	readonly getCreationAvailability: (intent: TabStripTabCreationIntent) => ActionAvailability;
	readonly createTab: (
		input: TabCreateInput,
		source: TabStripNewTabRequest['source'],
	) => ActionExecution;
	readonly execute: <Id extends ActionCatalogId<TabStripActionCatalog>>(
		actionId: Id,
		input: ActionInputFor<TabStripActionCatalog, Id>,
		source: ActionInvocationSource,
	) => ActionExecution<ActionOutputFor<TabStripActionCatalog, Id>>;
	readonly scope: ActionScope;
}
