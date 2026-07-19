import * as React from 'react';

import { WorkbenchShell, type WorkbenchLayoutStorage } from '../workbench-shell';

export interface WorkbenchProps {
	readonly layoutStorage?: WorkbenchLayoutStorage | undefined;
}

export function Workbench({ layoutStorage }: WorkbenchProps): React.ReactNode {
	return <WorkbenchShell layoutStorage={layoutStorage} />;
}
