import { createOrchbeeIntl, workbenchTabMessages, type OrchbeeLocale } from '@orchbee/i18n';
import { WorkbenchProjectWorkspaceRuntime } from '@orchbee/workbench';
import * as React from 'react';

export function useWebApplicationProjectWorkspace(
	locale: OrchbeeLocale,
): WorkbenchProjectWorkspaceRuntime {
	const intl = React.useMemo(() => createOrchbeeIntl(locale), [locale]);
	const [runtime] = React.useState(() => {
		const projectWorkspaceRuntime = new WorkbenchProjectWorkspaceRuntime({
			standaloneCwd: '/workspace',
			standaloneTerminalTitle: intl.formatMessage(workbenchTabMessages.terminalTitle, {
				number: 1,
			}),
		});
		return projectWorkspaceRuntime;
	});
	return runtime;
}
