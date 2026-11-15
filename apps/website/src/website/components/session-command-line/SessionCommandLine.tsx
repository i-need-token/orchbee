import { useOrchbeeIntl, websiteProductMessages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { useSessionFrame } from '../session-scene/useSessionFrame';
import { ShellWindow } from '../shell-window/ShellWindow';

/**
 * The Session seen from a terminal outside Orchbee: the `orchbee` commands that list the Agents
 * and read what one of them printed. Drawn in its finished state and bound to the Session's keys,
 * so it plays inside a SessionScene.
 */
export function SessionCommandLine(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	const frame = useSessionFrame();
	return (
		<ShellWindow
			label={t(websiteProductMessages.commandLine)}
			size='full'
			rows={frame.rows.cli}
			bind='cli'
		/>
	);
}
