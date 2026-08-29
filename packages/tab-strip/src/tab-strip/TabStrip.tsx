import type { ReactNode } from 'react';
import { TabStripView } from './components/tab-strip-view/TabStripView';
import { DEFAULT_TAB_STRIP_MESSAGES } from './constants/default-tab-strip-messages';
import { TabStripMessagesContext } from './contexts/tab-strip-messages-context';
import { useTabStrip } from './hooks/useTabStrip';
import type { TabStripMessages } from './types/tab-strip-messages';
import type {
	TabStripCrossWindowDragRuntime,
	TabStripCrossWindowDragRuntimeFactory,
	TabStripCloseCallback,
	TabStripCloseConfirmation,
	TabStripCloseRequest,
	TabStripNewTabRequest,
	TabStripTabCreationIntent,
	TabStripOptions,
	TabStripMoveGroupToWindowCallback,
	TabStripMoveTabsToWindowCallback,
	TabStripSelectCallback,
	TabStripShortcutLabels,
	TabStatusAdornment,
	TabStatusAdornmentKind,
	TabStripWindowTarget,
} from './types/tab-strip-options';

export interface TabStripProps extends TabStripOptions {
	horizontalLeadingActions?: ReactNode;
	messages?: TabStripMessages;
}

export type {
	TabStripCrossWindowDragRuntime,
	TabStripCrossWindowDragRuntimeFactory,
	TabStripCloseCallback,
	TabStripCloseConfirmation,
	TabStripCloseRequest,
	TabStripNewTabRequest,
	TabStripMoveGroupToWindowCallback,
	TabStripMoveTabsToWindowCallback,
	TabStripTabCreationIntent,
	TabStripSelectCallback,
	TabStripShortcutLabels,
	TabStatusAdornment,
	TabStatusAdornmentKind,
	TabStripWindowTarget,
	TabStripMessages,
};

export function TabStrip({
	horizontalLeadingActions = null,
	messages = DEFAULT_TAB_STRIP_MESSAGES,
	...options
}: TabStripProps): ReactNode {
	const view = useTabStrip(options, messages);

	return (
		<TabStripMessagesContext.Provider value={messages}>
			<TabStripView horizontalLeadingActions={horizontalLeadingActions} view={view} />
		</TabStripMessagesContext.Provider>
	);
}
