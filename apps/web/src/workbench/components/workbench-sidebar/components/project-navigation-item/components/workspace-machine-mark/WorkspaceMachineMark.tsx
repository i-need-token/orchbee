import type { ReactNode } from 'react';

import { machineColorValue } from '../../../../../../../machine-color';
import type { WorkbenchMachineMark } from '../../../../../../types/workbench-other-machines';
import styles from './WorkspaceMachineMark.module.css';

/**
 * `@` and the machine's name one space after a Workspace's own name, in regular weight and the
 * machine's color, as the TUI's sidebar marks a Workspace on another machine. A long name shortens
 * before the Workspace's does.
 */
export function WorkspaceMachineMark({ mark }: { readonly mark: WorkbenchMachineMark }): ReactNode {
	return (
		<span
			className={styles['mark']}
			style={{ '--ob-comp-machine-color': machineColorValue(mark.color) }}
		>
			@{mark.name}
		</span>
	);
}
