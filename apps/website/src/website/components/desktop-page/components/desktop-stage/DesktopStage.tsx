import { useOrchbeeIntl, websiteDesktopMessages as messages } from '@orchbee/i18n';
import { orchbeeTerminalColorTheme } from '@orchbee/terminal';
import type { ReactElement } from 'react';

import { richText } from '../../../../rich-text';
import { PageStage } from '../../../page-stage/PageStage';
import { SessionDesktopApplication } from '../../../session-desktop-application/SessionDesktopApplication';
import { SessionScene } from '../../../session-scene/SessionScene';
import { StateLegend } from '../../../state-legend/StateLegend';
import { terminalThemeData } from '../../../terminal-text/terminalThemeData';
import { OptionSwitch } from '../option-switch/OptionSwitch';
import styles from './DesktopStage.module.css';

/**
 * The scripted Session in the desktop application under the page head, with the switch between
 * the window's dark and light appearance above it and the caption with the legend of Agent states
 * under it. Each button of the switch carries its Orchbee terminal theme as page data. Without
 * script the window stays dark and the switch hidden.
 */
export function DesktopStage(): ReactElement {
	const { formatMessage: t } = useOrchbeeIntl();
	return (
		<SessionScene>
			<div className={styles['stage']} data-desktop-stage=''>
				<OptionSwitch
					name='appearance'
					label={t(messages.appearance)}
					options={[
						{
							id: 'dark',
							label: t(messages.appearanceDark),
							theme: terminalThemeData(orchbeeTerminalColorTheme('dark')),
						},
						{
							id: 'light',
							label: t(messages.appearanceLight),
							theme: terminalThemeData(orchbeeTerminalColorTheme('light')),
						},
					]}
					pressed='dark'
					note={t(messages.appearanceNote)}
					placement='above'
				/>
				<PageStage
					caption={t(messages.stageCaption, richText)}
					aside={<StateLegend states={['working', 'attention', 'done', 'idle']} />}
				>
					<SessionDesktopApplication />
				</PageStage>
			</div>
		</SessionScene>
	);
}
