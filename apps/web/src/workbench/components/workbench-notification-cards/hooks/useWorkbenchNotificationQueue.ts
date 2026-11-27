import * as React from 'react';

import { WorkbenchNotificationQueue } from '../model/WorkbenchNotificationQueue';

/** One notification queue for the Workbench's lifetime; unmounting dismisses its cards. */
export function useWorkbenchNotificationQueue(): WorkbenchNotificationQueue {
	const [queue] = React.useState(() => new WorkbenchNotificationQueue());
	React.useEffect(() => () => queue.close(), [queue]);
	return queue;
}
