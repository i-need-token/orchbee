import { createOrchbeeIntl, workbenchTabMessages, type OrchbeeLocale } from '@orchbee/i18n';
import {
	WorkbenchProjectWorkspaceRuntime,
	createWorkbenchPreviewTerminalPresentationComposition,
	terminalPresentationId,
} from '@orchbee/workbench';
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
			terminalPresentation: createWorkbenchPreviewTerminalPresentationComposition(
				terminalPresentationId('web:standalone:initial'),
			),
		});
		return projectWorkspaceRuntime;
	});
	return runtime;
}
