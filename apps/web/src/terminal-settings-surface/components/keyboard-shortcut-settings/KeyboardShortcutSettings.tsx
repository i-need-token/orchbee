import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { Button, Kbd } from '@orchbee/ui/components';
import * as React from 'react';

import type { KeyboardShortcutSettingsController } from '../../../keyboard-shortcuts';
import { useKeyboardShortcutSettingsView } from './hooks/useKeyboardShortcutSettingsView';
import styles from './KeyboardShortcutSettings.module.css';

interface KeyboardShortcutSettingsProps {
	readonly controller: KeyboardShortcutSettingsController;
	readonly platform: KeyboardShortcutPlatform;
}

export function KeyboardShortcutSettings({
	controller,
	platform,
}: KeyboardShortcutSettingsProps): React.ReactNode {
	const view = useKeyboardShortcutSettingsView({ controller, platform });

	return (
		<div className={styles['root']} data-slot='keyboard-shortcut-settings'>
			{view.persistenceWarning === null ? null : (
				<div className={styles['warning']} role='alert'>
					{view.persistenceWarning}
				</div>
			)}
			<div className={styles['list']}>
				{view.rows.map((row) => (
					<div
						className={styles['row']}
						data-shortcut-id={row.shortcutId}
						id={`keyboard-shortcuts.${row.shortcutId}`}
						key={row.shortcutId}
					>
						<div className={styles['copy']}>
							<div className={styles['actionName']}>{row.actionName}</div>
							<div className={styles['metadata']}>
								<span>{row.scope}</span>
								<span aria-hidden='true'>·</span>
								<span>{row.availability}</span>
							</div>
							{row.recording ? (
								<div className={styles['recording']} role='status'>
									{view.recordingInstruction}
								</div>
							) : null}
							{row.error === null ? null : (
								<div className={styles['error']} role='alert'>
									{row.error}
								</div>
							)}
						</div>
						<div className={styles['controls']}>
							<span className={styles['source']} data-source={row.sourceKind}>
								{row.source}
							</span>
							<button
								aria-label={view.changeLabel(row.actionName)}
								aria-pressed={row.recording}
								className={styles['recorder']}
								data-recording={row.recording}
								type='button'
								onBlur={row.onBlur}
								onClick={row.onChangeClick}
								onKeyDown={row.onKeyDown}
							>
								<Kbd>{row.chord}</Kbd>
							</button>
							{row.sourceKind === 'custom' ? (
								<Button size='sm' variant='ghost' onClick={row.onReset}>
									{row.resetLabel}
								</Button>
							) : null}
						</div>
					</div>
				))}
			</div>
		</div>
	);
}
