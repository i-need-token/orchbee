import type { ReactNode } from 'react';
import { TabStripView } from './components/tab-strip-view/TabStripView';
import { DEFAULT_TAB_STRIP_MESSAGES } from './constants/default-tab-strip-messages';
import { TabStripMessagesContext } from './contexts/tab-strip-messages-context';
import { useTabStrip } from './hooks/useTabStrip';
import type { TabStripMessages } from './types/tab-strip-messages';
import type {
	TabStripArrangementChange,
	TabStripChangeCallback,
	TabStripCloseCallback,
	TabStripCloseResume,
	TabStripGroupNameRequest,
	TabStripNewTabRequest,
	TabStripTabCreationIntent,
	TabStripOptions,
	TabStripSelectCallback,
	TabStripNewTabChoice,
	TabStripNewTabSection,
	TabStatusAdornment,
	TabStatusAdornmentKind,
} from './types/tab-strip-options';

interface TabStripProps extends TabStripOptions {
	horizontalLeadingActions?: ReactNode;
	horizontalTrailingActions?: ReactNode;
	messages?: TabStripMessages;
}

export type {
	TabStripArrangementChange,
	TabStripChangeCallback,
	TabStripCloseCallback,
	TabStripCloseResume,
	TabStripGroupNameRequest,
	TabStripNewTabRequest,
	TabStripTabCreationIntent,
	TabStripSelectCallback,
	TabStripNewTabChoice,
	TabStripNewTabSection,
	TabStatusAdornment,
	TabStatusAdornmentKind,
	TabStripMessages,
};

export function TabStrip({
	horizontalLeadingActions = null,
	horizontalTrailingActions = null,
	messages = DEFAULT_TAB_STRIP_MESSAGES,
	...options
}: TabStripProps): ReactNode {
	const view = useTabStrip(options, messages);

	return (
		<TabStripMessagesContext.Provider value={messages}>
			<TabStripView
				horizontalLeadingActions={horizontalLeadingActions}
				horizontalTrailingActions={horizontalTrailingActions}
				view={view}
			/>
		</TabStripMessagesContext.Provider>
	);
}
