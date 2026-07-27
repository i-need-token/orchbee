import { ActionRuntime } from '@orchbee/actions';
import * as React from 'react';

export function useWebApplicationActionRuntime(): ActionRuntime {
	const [runtime] = React.useState(() => new ActionRuntime());
	const lifecycleRevision = React.useRef(0);

	React.useEffect(() => {
		lifecycleRevision.current += 1;
		const setupRevision = lifecycleRevision.current;
		return () => {
			queueMicrotask(() => {
				if (lifecycleRevision.current === setupRevision) {
					runtime.dispose();
				}
			});
		};
	}, [runtime]);

	return runtime;
}
