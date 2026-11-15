import { useOrchbeeIntl, websiteProductMessages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { ShellWindow } from '../../../../../shell-window/ShellWindow';
import type { MultiplexingFrame } from '../../multiplexing-scene';

/** A short console outside Orchbee that reads the shared shell's last rows. */
export function MultiplexingCommandLine({ frame }: { frame: MultiplexingFrame }): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<ShellWindow
			label={t(websiteProductMessages.commandLine)}
			size='strip'
			rows={frame.rows.reader}
			bind='reader'
		/>
	);
}
