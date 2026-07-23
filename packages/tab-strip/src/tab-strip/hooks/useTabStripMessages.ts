import { useContext } from 'react';

import { TabStripMessagesContext } from '../contexts/tab-strip-messages-context';
import type { TabStripMessages } from '../types/tab-strip-messages';

export function useTabStripMessages(): TabStripMessages {
	return useContext(TabStripMessagesContext);
}
