import type { ReactNode } from 'react';
import { TabStripView } from './components/tab-strip-view/TabStripView';
import { DEFAULT_TAB_STRIP_MESSAGES } from './constants/default-tab-strip-messages';
import { TabStripMessagesContext } from './contexts/tab-strip-messages-context';
import { useTabStrip } from './hooks/useTabStrip';
import type { TabStripMessages } from './types/tab-strip-messages';
import type { TabStripCrossWindowDragRuntime, TabStripOptions } from './types/tab-strip-options';

export interface TabStripProps extends TabStripOptions {
	messages?: TabStripMessages;
}

export type { TabStripCrossWindowDragRuntime, TabStripMessages };

export function TabStrip({
	messages = DEFAULT_TAB_STRIP_MESSAGES,
	...options
}: TabStripProps): ReactNode {
	const view = useTabStrip(options, messages);

	return (
		<TabStripMessagesContext.Provider value={messages}>
			<TabStripView view={view} />
		</TabStripMessagesContext.Provider>
	);
}
