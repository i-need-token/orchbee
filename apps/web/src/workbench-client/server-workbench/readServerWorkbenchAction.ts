import { parseWorkbenchAction } from '@orchbee/workbench-client-protocol';

import type { ServerWorkbenchAction } from './ServerWorkbenchSession';

/** Text a person typed can fall outside what the Server accepts, which is a refusal, not a defect. */
export function readServerWorkbenchAction(
	input: Parameters<typeof parseWorkbenchAction>[0],
): ServerWorkbenchAction | null {
	try {
		return parseWorkbenchAction(input);
	} catch {
		return null;
	}
}
