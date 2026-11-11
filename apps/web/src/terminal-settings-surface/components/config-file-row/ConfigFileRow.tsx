import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import { Button } from '@orchbee/ui/components';
import * as React from 'react';

import { SettingsGroup } from '../settings-group/SettingsGroup';
import { SettingsRow } from '../settings-row/SettingsRow';

interface ConfigFileRowProps {
	/** The settings file this Session reads, from its settings snapshot; null until one arrives. */
	readonly filePath: string | null;
	readonly query: string;
}

/**
 * The last card of a category whose remaining options live only in the configuration file. It
 * names the file the Session actually reads and copies that path; it never opens the file.
 */
export function ConfigFileRow({ filePath, query }: ConfigFileRowProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const [copied, setCopied] = React.useState(false);
	const copy = (): void => {
		if (filePath === null) return;
		void navigator.clipboard.writeText(filePath).then(
			() => setCopied(true),
			() => setCopied(false),
		);
	};
	return (
		<SettingsGroup>
			<SettingsRow
				detail={filePath}
				label={intl.formatMessage(terminalSettingsMessages.pageConfigFileLabel)}
				query={query}
				searchTerms={['config.toml']}
				settingId='settings.config-file'
			>
				<Button
					disabled={filePath === null}
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
