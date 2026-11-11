import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import * as React from 'react';

import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SettingsRow } from '../settings-row/SettingsRow';

/**
 * The release configuration file. The renderer has no port that reveals or opens the active file,
 * so the row names the release location and copies it.
 */
const CONFIG_FILE_PATH = '~/.orchbee/config.toml';

/** The last card of a category whose remaining options live only in the configuration file. */
export function ConfigFileRow({ query }: { readonly query: string }): React.ReactNode {
	const intl = useOrchbeeIntl();
	const [copied, setCopied] = React.useState(false);
	const copy = (): void => {
		void navigator.clipboard.writeText(CONFIG_FILE_PATH).then(
			() => setCopied(true),
			() => setCopied(false),
		);
	};
	return (
		<SettingsGroup>
			<SettingsRow
				detail={CONFIG_FILE_PATH}
				label={intl.formatMessage(terminalSettingsMessages.pageConfigFileLabel)}
				query={query}
				searchTerms={['config.toml']}
				settingId='settings.config-file'
			>
				<Button
					size='sm'
					type='button'
					variant='outline'
					onClick={copy}
					onBlur={() => setCopied(false)}
				>
					{intl.formatMessage(
						copied
							? terminalSettingsMessages.pageConfigFileCopied
							: terminalSettingsMessages.pageConfigFileCopy,
					)}
				</Button>
			</SettingsRow>
		</SettingsGroup>
	);
}
