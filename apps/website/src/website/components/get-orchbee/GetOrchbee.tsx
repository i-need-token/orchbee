import { useOrchbeeIntl, websiteShellMessages as messages } from '@orchbee/i18n';
import type { ReactElement } from 'react';

import { BUILD_GUIDE_URL, NPM_INSTALL_COMMAND, type Release } from '../../release';
import { CommandChip } from '../command-chip/CommandChip';
import { LinkButton } from '../link-button/LinkButton';
import styles from './GetOrchbee.module.css';

// The way to get Orchbee that a call to action offers. Once a release is published it offers the
// Windows installer, the npm command, or both; before that, every call to action leads to the
// source build.
export function GetOrchbee({
	release,
	offer,
}: {
	release: Release;
	offer: 'both' | 'installer' | 'npm';
}): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<div className={styles['actions']}>
			{release.published ? (
				<>
					{offer === 'npm' ? null : (
						<LinkButton href={release.downloads['windows-x86_64']} icon='download'>
							{t(messages.downloadForWindows)}
						</LinkButton>
					)}
					{offer === 'installer' ? null : <CommandChip command={NPM_INSTALL_COMMAND} />}
				</>
			) : (
				<LinkButton href={BUILD_GUIDE_URL}>{t(messages.getSource)}</LinkButton>
			)}
		</div>
	);
}
