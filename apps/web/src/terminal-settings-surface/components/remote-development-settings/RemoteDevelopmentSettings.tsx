import { Textarea } from '@orchbee/ui/components';
import * as React from 'react';

import {
	REMOTE_DEVELOPMENT_BASELINE_TERM,
	REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND,
} from '../../constants/remote-development-diagnostic';
import type { TerminalSettingsSurfaceMessages } from '../../types/terminal-settings-surface';
import styles from './RemoteDevelopmentSettings.module.css';

interface RemoteDevelopmentSettingsProps {
	readonly messages: Pick<
		TerminalSettingsSurfaceMessages,
		| 'remoteDevelopmentBaselineDescription'
		| 'remoteDevelopmentBaselineTitle'
		| 'remoteDevelopmentDiagnosticDescription'
		| 'remoteDevelopmentDiagnosticLabel'
		| 'remoteDevelopmentDiagnosticTitle'
		| 'remoteDevelopmentNoChangesDescription'
		| 'remoteDevelopmentNoChangesTitle'
		| 'remoteDevelopmentResultDescription'
	>;
}

export function RemoteDevelopmentSettings({
	messages,
}: RemoteDevelopmentSettingsProps): React.ReactNode {
	return (
		<section className={styles['root']} data-slot='remote-development-settings'>
			<section
				className={styles['card']}
				id='remote-development.baseline'
				aria-labelledby='remote-development-baseline-title'
			>
				<h2 className={styles['title']} id='remote-development-baseline-title'>
					{messages.remoteDevelopmentBaselineTitle}
				</h2>
				<p className={styles['description']}>{messages.remoteDevelopmentBaselineDescription}</p>
				<code className={styles['term']}>{REMOTE_DEVELOPMENT_BASELINE_TERM}</code>
			</section>

			<section
				className={styles['card']}
				id='remote-development.diagnostic'
				aria-labelledby='remote-development-diagnostic-title'
			>
				<h2 className={styles['title']} id='remote-development-diagnostic-title'>
					{messages.remoteDevelopmentDiagnosticTitle}
				</h2>
				<p className={styles['description']} id='remote-development-diagnostic-description'>
					{messages.remoteDevelopmentDiagnosticDescription}
				</p>
				<label className={styles['label']} htmlFor='remote-development-diagnostic-command'>
					{messages.remoteDevelopmentDiagnosticLabel}
				</label>
				<Textarea
					readOnly
					aria-describedby='remote-development-diagnostic-description remote-development-result-description'
					className={styles['command']}
					id='remote-development-diagnostic-command'
					rows={9}
					spellCheck={false}
					value={REMOTE_DEVELOPMENT_DIAGNOSTIC_COMMAND}
				/>
				<p className={styles['description']} id='remote-development-result-description'>
					{messages.remoteDevelopmentResultDescription}
				</p>
			</section>

			<aside
				className={styles['notice']}
				id='remote-development.no-automatic-changes'
				aria-labelledby='remote-development-no-changes-title'
			>
				<h2 className={styles['noticeTitle']} id='remote-development-no-changes-title'>
					{messages.remoteDevelopmentNoChangesTitle}
				</h2>
				<p className={styles['description']}>{messages.remoteDevelopmentNoChangesDescription}</p>
			</aside>
		</section>
	);
}
