import { useContext } from 'react';

import { TabStripMessagesContext } from '../tab-strip-messages-context';
import type { TabStripMessages } from '../tab-strip-messages';

export function useTabStripMessages(): TabStripMessages {
	return useContext(TabStripMessagesContext);
}
