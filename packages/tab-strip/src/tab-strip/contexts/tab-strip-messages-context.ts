import { createContext } from 'react';

import { DEFAULT_TAB_STRIP_MESSAGES } from '../constants/default-tab-strip-messages';
import type { TabStripMessages } from '../types/tab-strip-messages';

export const TabStripMessagesContext = createContext<TabStripMessages>(DEFAULT_TAB_STRIP_MESSAGES);
