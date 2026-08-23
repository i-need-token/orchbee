import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { Button, Input, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import { GhosttyMigrationSettings } from './components/ghostty-migration-settings/GhosttyMigrationSettings';
import { KeyboardShortcutSettings } from './components/keyboard-shortcut-settings/KeyboardShortcutSettings';
import { MacosTerminalInputSettings } from './components/macos-terminal-input-settings/MacosTerminalInputSettings';
import { PlannedTerminalSettings } from './components/planned-terminal-settings/PlannedTerminalSettings';
import { RemoteDevelopmentSettings } from './components/remote-development-settings/RemoteDevelopmentSettings';
import { ShellIntegrationSettings } from './components/shell-integration-settings/ShellIntegrationSettings';
import { TerminalSettingsSections } from './components/terminal-settings-sections/TerminalSettingsSections';
import { TerminalLaunchProfileSettings } from './components/terminal-launch-profile-settings/TerminalLaunchProfileSettings';
import { useTerminalSettingsSurface } from './hooks/useTerminalSettingsSurface';
import type { TerminalSettingsController } from '../terminal-settings';
import type { TerminalInteractionSettingsController } from '../terminal-interaction-settings';
import type { KeyboardShortcutSettingsController } from '../keyboard-shortcuts';
import type { TerminalFeedbackSettingsController } from '../terminal-feedback-settings';
import type { MacosTerminalInputSettingsController } from '../macos-terminal-input-settings';
import type { DesktopTerminalLaunchProfileControl } from '../terminal-launch-profile-control';
import type { DesktopTerminalDefaultSettingsControl } from '../terminal-default-settings-control';
import styles from './TerminalSettingsSurface.module.css';

export interface TerminalSettingsSurfaceProps {
	readonly controller: TerminalSettingsController;
	readonly keyboardShortcutController: KeyboardShortcutSettingsController;
	readonly keyboardShortcutPlatform: KeyboardShortcutPlatform;
	readonly macosTerminalInputSettingsController?: MacosTerminalInputSettingsController | undefined;
	readonly onRequestClose: () => void;
	readonly requestBrowserAccess?: ((returnFocus: HTMLElement) => void) | undefined;
	readonly terminalFeedbackSettingsController?: TerminalFeedbackSettingsController | undefined;
	readonly terminalLaunchProfileClient?: DesktopTerminalLaunchProfileControl | undefined;
	readonly terminalDefaultSettingsControl?: DesktopTerminalDefaultSettingsControl | undefined;
	readonly terminalInteractionSettingsController?:
		| TerminalInteractionSettingsController
		| undefined;
}

export function TerminalSettingsSurface(props: TerminalSettingsSurfaceProps): React.ReactNode {
	const view = useTerminalSettingsSurface(props);
	if (!view.open) return null;
	const appearancePreferenceCategory =
		view.category === 'appearance' ||
		view.category === 'colors' ||
		view.category === 'general' ||
		view.category === 'interaction' ||
		view.category === 'text';
	const terminalSettingsSectionsCategory =
		appearancePreferenceCategory || view.category === 'terminal';

	let mainContent: React.ReactNode;
	if (view.category === 'keyboard-shortcuts') {
		mainContent = (
			<KeyboardShortcutSettings
				controller={props.keyboardShortcutController}
				platform={props.keyboardShortcutPlatform}
			/>
		);
	} else if (view.category === 'ghostty-migration') {
		mainContent = <GhosttyMigrationSettings />;
	} else if (view.category === 'browser-access') {
		mainContent = (
			<div className={styles['browserAccessAction']} id='browser-access.open'>
				<Button onClick={view.onOpenBrowserAccess}>
					<ProductIcon name='screen-full' />
					{view.messages.openBrowserAccess}
				</Button>
			</div>
		);
	} else if (view.category === 'remote-development') {
		mainContent = <RemoteDevelopmentSettings messages={view.messages} />;
	} else if (view.category === 'profiles' && props.terminalLaunchProfileClient !== undefined) {
		mainContent = (
			<div className={styles['settingsStack']}>
				<TerminalLaunchProfileSettings client={props.terminalLaunchProfileClient} />
				<PlannedTerminalSettings notice={view.plannedNotice} sections={view.plannedSections} />
			</div>
		);
	} else if (
		view.category === 'shell-integration' &&
		props.terminalFeedbackSettingsController !== undefined
	) {
		mainContent = (
			<ShellIntegrationSettings controller={props.terminalFeedbackSettingsController} />
		);
	} else {
		const actualSettings = (
			<>
				{view.category !== 'cursor-input' ||
				props.macosTerminalInputSettingsController === undefined ||
				!view.macosInputVisible ? null : (
					<MacosTerminalInputSettings controller={props.macosTerminalInputSettingsController} />
				)}
				{terminalSettingsSectionsCategory ? <TerminalSettingsSections view={view} /> : null}
			</>
		);
		mainContent = (
			<div className={styles['settingsStack']}>
				{view.category === 'general' ? null : actualSettings}
				<PlannedTerminalSettings notice={view.plannedNotice} sections={view.plannedSections} />
				{view.category === 'general' ? actualSettings : null}
			</div>
		);
	}

	return (
		<div
			aria-labelledby='terminal-settings-title'
			aria-modal='true'
			className={styles['root']}
			data-slot='terminal-settings-surface'
			role='dialog'
			onKeyDown={view.onSurfaceKeyDown}
		>
			<div aria-hidden='true' className={styles['windowDragRegion']} />
			<aside className={styles['sidebar']}>
				<button
					aria-label={view.messages.back}
					className={styles['sidebarHeader']}
					data-settings-back
					title={view.messages.back}
					type='button'
					onClick={view.onRequestClose}
				>
					<span aria-hidden='true' className={styles['back']}>
						<ProductIcon name='arrow-left' />
					</span>
					<span className={styles['sidebarTitle']}>{view.messages.title}</span>
				</button>
				<div className={styles['search']}>
					<ProductIcon name='search' />
					<Input
						ref={view.inputReference}
						aria-activedescendant={view.activeSearchResultId}
						aria-autocomplete='list'
						aria-controls={view.searchActive ? view.searchResultsId : undefined}
						aria-expanded={view.searchActive}
						aria-label={view.messages.searchPlaceholder}
						autoComplete='off'
						className={styles['searchInput']}
						placeholder={view.messages.searchPlaceholder}
						role='combobox'
						value={view.query}
						onChange={view.onSearchChange}
						onKeyDown={view.onSearchKeyDown}
					/>
				</div>
				{view.searchActive ? (
					<div
						aria-label={view.messages.searchPlaceholder}
						aria-live={view.searchResults.length === 0 ? 'polite' : undefined}
						className={styles['searchResults']}
						id={view.searchResultsId}
						role='listbox'
					>
						{view.searchResults.length === 0 ? (
							<p
								aria-disabled='true'
								aria-selected='false'
								className={styles['searchEmpty']}
								role='option'
							>
								{view.messages.noResults}
							</p>
						) : (
							view.searchResults.map((result) => (
								<button
									aria-selected={result.active}
									className={styles['searchResult']}
									data-active={result.active}
									data-settings-search-result={result.id}
									id={result.elementId}
									key={result.id}
									role='option'
									type='button'
									onClick={result.onActivate}
									onFocus={result.onFocus}
									onMouseEnter={result.onFocus}
								>
									<span className={styles['categoryIcon']}>
										<ProductIcon name={result.icon} />
									</span>
									<span className={styles['searchResultLabel']}>
										{result.kind === 'category' ? null : (
											<>
												<span className={styles['searchResultCategory']}>
													{result.categoryLabel}
												</span>
												<span aria-hidden='true' className={styles['searchResultSeparator']}>
													›
												</span>
											</>
										)}
										<span className={styles['searchResultSetting']}>{result.label}</span>
									</span>
									{result.statusLabel === null ? null : (
										<span className={styles['searchResultStatus']}>{result.statusLabel}</span>
									)}
								</button>
							))
						)}
					</div>
				) : (
					<nav aria-label={view.messages.title} className={styles['categories']}>
						{view.navigationGroups.map((group) => (
							<div
								className={styles['categoryGroup']}
								data-settings-group={group.id}
								key={group.id}
							>
								<div className={styles['categoryGroupLabel']}>{group.label}</div>
								{group.items.map((item) => (
									<button
										aria-current={view.category === item.id ? 'page' : undefined}
										className={styles['category']}
										data-selected={view.category === item.id}
										data-settings-category={item.id}
										key={item.id}
										type='button'
										onClick={(): void => view.onCategorySelect(item.id)}
									>
										<span className={styles['categoryIcon']}>
											<ProductIcon name={item.icon} />
										</span>
										<span className={styles['categoryLabel']}>{item.label}</span>
									</button>
								))}
							</div>
						))}
					</nav>
				)}
			</aside>

			<main
				ref={view.contentReference}
				className={styles['content']}
				data-settings-content={view.category}
				tabIndex={0}
			>
				<div className={styles['contentColumn']}>
					<header className={styles['header']}>
						<h1 className={styles['title']} id='terminal-settings-title'>
							{view.pageTitle}
						</h1>
						<p className={styles['description']}>{view.pageDescription}</p>
					</header>

					{!appearancePreferenceCategory || view.persistenceWarning === null ? null : (
						<div className={styles['warning']} role='alert'>
							{view.persistenceWarning}
						</div>
					)}
					{view.category !== 'terminal' || view.terminalDefaultSettingsWarning === null ? null : (
						<div className={styles['warning']} role='alert'>
							{view.terminalDefaultSettingsWarning}
						</div>
					)}

					{mainContent}
				</div>
			</main>
		</div>
	);
}
