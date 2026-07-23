import { createContext } from 'react';

import { DEFAULT_TAB_STRIP_MESSAGES, type TabStripMessages } from './tab-strip-messages';

export const TabStripMessagesContext = createContext<TabStripMessages>(DEFAULT_TAB_STRIP_MESSAGES);
