import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import {
	Button,
	InputGroup,
	InputGroupAddon,
	InputGroupButton,
	InputGroupInput,
	Kbd,
	ProductIcon,
} from '@orchbee/ui/components';
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
			<div className={styles['discovery']} data-slot='keyboard-shortcut-discovery'>
				<div className={styles['searchControls']}>
					<InputGroup data-recording={view.search.recording}>
						<InputGroupAddon>
							<ProductIcon name='search' />
						</InputGroupAddon>
						<InputGroupInput
							ref={view.search.inputReference}
							aria-describedby={
								view.search.status === null ? undefined : 'keyboard-shortcut-search-status'
							}
							aria-label={view.search.label}
							autoComplete='off'
							placeholder={view.search.placeholder}
							readOnly={view.search.recording}
							spellCheck={false}
							value={view.search.query}
							onChange={view.search.onChange}
							onKeyDown={view.search.onKeyDown}
						/>
						{view.search.query.length === 0 ? null : (
							<InputGroupAddon align='inline-end'>
								<InputGroupButton
									aria-label={view.search.clearLabel}
									size='icon-xs'
									onClick={view.search.onClear}
								>
									<ProductIcon name='close' />
								</InputGroupButton>
							</InputGroupAddon>
						)}
					</InputGroup>
					<Button
						aria-pressed={view.search.recording}
						data-slot='keyboard-shortcut-record-keys'
						variant={view.search.recording ? 'secondary' : 'outline'}
						onClick={view.search.onRecordingToggle}
					>
						{view.search.recordingToggleLabel}
					</Button>
				</div>
				{view.search.status === null ? null : (
					<p
						aria-live='polite'
						className={styles['searchStatus']}
						id='keyboard-shortcut-search-status'
						role='status'
					>
						{view.search.status}
					</p>
				)}
			</div>
			{view.rows.length === 0 ? (
				<div className={styles['empty']}>{view.noResults}</div>
			) : (
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
			)}
		</div>
	);
}
