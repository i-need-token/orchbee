import { createContext } from 'react';

import type { WorkbenchKeyboardScopeRegistry } from './WorkbenchKeyboardScopeRegistry';

/** The Workbench's scope registry; outside a Workbench, scopes register nowhere. */
export const WorkbenchKeyboardScopeContext = createContext<WorkbenchKeyboardScopeRegistry | null>(
	null,
);
