import * as React from 'react';

import { WorkbenchQuickInputSessions } from '../model/WorkbenchQuickInputSessions';

/** One quick input coordinator for the Workbench's lifetime; unmounting cancels its session. */
export function useWorkbenchQuickInputSessions(): WorkbenchQuickInputSessions {
	const [sessions] = React.useState(() => new WorkbenchQuickInputSessions());
	React.useEffect(() => () => sessions.close(), [sessions]);
	return sessions;
}
