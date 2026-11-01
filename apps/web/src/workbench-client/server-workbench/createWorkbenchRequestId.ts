import {
	WorkbenchRequestIdSchema,
	type WorkbenchCommand,
} from '@orchbee/workbench-client-protocol';

import { createClientServerRequestId } from '../server-connection';

export function createWorkbenchRequestId(): WorkbenchCommand['requestId'] {
	return WorkbenchRequestIdSchema.parse(createClientServerRequestId());
}
