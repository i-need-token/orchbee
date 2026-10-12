import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { Input, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import type {
	AgentIntegrationManagementControl,
	AgentIntegrationReminderController,
} from '../agent-integration-management';
import type { ApplicationLocaleController } from '../application-locale';
import type { KeyboardShortcutSettingsController } from '../keyboard-shortcuts';
import type { MacosTerminalInputSettingsController } from '../macos-terminal-input-settings';

import type { DesktopTerminalDefaultSettingsControl } from '../terminal-default-settings-control';
import type { TerminalFeedbackSettingsController } from '../terminal-feedback-settings';
import type { TerminalInteractionSettingsController } from '../terminal-interaction-settings';
import type { DesktopTerminalLaunchProfileControl } from '../terminal-launch-profile-control';
import type { TerminalSettingsController } from '../terminal-settings';
import { AgentIntegrationSettings } from './components/agent-integration-settings/AgentIntegrationSettings';
import { ApplicationLanguageSettings } from './components/application-language-settings/ApplicationLanguageSettings';
import { GhosttyMigrationSettings } from './components/ghostty-migration-settings/GhosttyMigrationSettings';
import { KeyboardShortcutSettings } from './components/keyboard-shortcut-settings/KeyboardShortcutSettings';
import { MacosTerminalInputSettings } from './components/macos-terminal-input-settings/MacosTerminalInputSettings';
import { RemoteDevelopmentSettings } from './components/remote-development-settings/RemoteDevelopmentSettings';
import { ShellIntegrationSettings } from './components/shell-integration-settings/ShellIntegrationSettings';

import { TerminalSettingsSections } from './components/terminal-settings-sections/TerminalSettingsSections';
import { TerminalLaunchProfileSettings } from './components/terminal-launch-profile-settings/TerminalLaunchProfileSettings';
import { useTerminalSettingsSurface } from './hooks/useTerminalSettingsSurface';
import type { TerminalSettingsCategory } from './types/terminal-settings-surface';
import styles from './TerminalSettingsSurface.module.css';
import { SharedPreferenceStatus } from './components/shared-preference-status/SharedPreferenceStatus';

export interface TerminalSettingsSurfaceProps {
	readonly agentIntegrationManagementControl?: AgentIntegrationManagementControl | undefined;
	readonly agentIntegrationReminderController?: AgentIntegrationReminderController | undefined;
	readonly applicationLocaleController: ApplicationLocaleController;
	readonly controller: TerminalSettingsController;
	readonly keyboardShortcutController: KeyboardShortcutSettingsController;
	readonly keyboardShortcutPlatform: KeyboardShortcutPlatform;
	readonly macosTerminalInputSettingsController?: MacosTerminalInputSettingsController | undefined;
	readonly onRequestClose: () => void;

	readonly requestedCategory?: TerminalSettingsCategory | undefined;

	readonly terminalFeedbackSettingsController?: TerminalFeedbackSettingsController | undefined;
	readonly terminalLaunchProfileClient?: DesktopTerminalLaunchProfileControl | undefined;
	readonly terminalDefaultSettingsControl?: DesktopTerminalDefaultSettingsControl | undefined;
	readonly terminalInteractionSettingsController?:
		| TerminalInteractionSettingsController
		| undefined;
}

export function TerminalSettingsSurface(props: TerminalSettingsSurfaceProps): React.ReactNode {
	const control = props.controller.userPreferences;
	const resetRevision = React.useSyncExternalStore(
		control?.subscribe ?? subscribeNoPreferences,
		() => control?.getSnapshot().draftResetRevision ?? 0,
		() => control?.getSnapshot().draftResetRevision ?? 0,
	);
	return (
		<TerminalSettingsSurfaceContent
			key={resetRevision}
			agentIntegrationManagementControl={props.agentIntegrationManagementControl}
			agentIntegrationReminderController={props.agentIntegrationReminderController}
			applicationLocaleController={props.applicationLocaleController}
			controller={props.controller}
			keyboardShortcutController={props.keyboardShortcutController}
			keyboardShortcutPlatform={props.keyboardShortcutPlatform}
			macosTerminalInputSettingsController={props.macosTerminalInputSettingsController}
			onRequestClose={props.onRequestClose}
			requestedCategory={props.requestedCategory}
			terminalFeedbackSettingsController={props.terminalFeedbackSettingsController}
			terminalLaunchProfileClient={props.terminalLaunchProfileClient}
			terminalDefaultSettingsControl={props.terminalDefaultSettingsControl}
			terminalInteractionSettingsController={props.terminalInteractionSettingsController}
		/>
	);
}

function subscribeNoPreferences(): () => void {
	return () => undefined;
}

function TerminalSettingsSurfaceContent(props: TerminalSettingsSurfaceProps): React.ReactNode {
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
	if (
		view.category === 'agent-integrations' &&
		props.agentIntegrationManagementControl !== undefined
	) {
		mainContent = (
			<AgentIntegrationSettings
				control={props.agentIntegrationManagementControl}
				reminderController={props.agentIntegrationReminderController}
			/>
		);
	} else if (view.category === 'keyboard-shortcuts') {
		mainContent = (
			<KeyboardShortcutSettings
				controller={props.keyboardShortcutController}
				platform={props.keyboardShortcutPlatform}
			/>
		);
	} else if (view.category === 'ghostty-migration') {
		mainContent = <GhosttyMigrationSettings />;
	} else if (view.category === 'remote-development') {
		mainContent = <RemoteDevelopmentSettings messages={view.messages} />;
	} else if (view.category === 'profiles' && props.terminalLaunchProfileClient !== undefined) {
		mainContent = <TerminalLaunchProfileSettings client={props.terminalLaunchProfileClient} />;
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
				{view.category === 'general' ? (
					<ApplicationLanguageSettings controller={props.applicationLocaleController} />
				) : (
					actualSettings
				)}
				{view.category === 'general' ? actualSettings : null}
			</div>
		);
	}

	return (
		<div
			className={styles['root']}
			data-mobile-navigation={view.navigationOpen ? 'navigation' : 'content'}
			data-slot='terminal-settings-surface'
			onKeyDown={view.onSurfaceKeyDown}
		>
			<div aria-hidden='true' className={styles['windowDragRegion']} />
			<div
				aria-labelledby='terminal-settings-title'
				aria-modal='true'
				className={styles['panel']}
				data-settings-layout='bounded'
				role='dialog'
			>
				<aside className={styles['sidebar']}>
					<div className={styles['sidebarHeader']}>
						<button
							aria-label={view.messages.close}
							className={styles['close']}
							data-settings-close
							title={view.messages.close}
							type='button'
							onClick={view.onRequestClose}
						>
							<ProductIcon name='close' />
						</button>
						<span className={styles['sidebarTitle']}>{view.messages.title}</span>
					</div>
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
									</button>
								))
							)}
						</div>
					) : (
						<nav aria-label={view.messages.title} className={styles['categories']}>
							{view.navigationGroups.map((group) => (
								<div
									aria-label={group.label}
									className={styles['categoryGroup']}
									data-settings-group={group.id}
									key={group.id}
									role='group'
								>
									<div
										aria-hidden='true'
										className={styles['categoryGroupLabel']}
										data-settings-group-label={group.id}
									>
										{group.label}
									</div>
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
						<button
							aria-label={view.messages.back}
							className={styles['mobileNavigationBack']}
							title={view.messages.back}
							type='button'
							onClick={view.onNavigationRequest}
						>
							<ProductIcon name='arrow-left' />
							<span>{view.messages.title}</span>
						</button>
						<header className={styles['header']}>
							<h1 className={styles['title']} id='terminal-settings-title' tabIndex={-1}>
								{view.pageTitle}
							</h1>
							<p className={styles['description']}>{view.pageDescription}</p>
							{props.controller.userPreferences === undefined ? null : (
								<SharedPreferenceStatus control={props.controller.userPreferences} />
							)}
						</header>

						{!appearancePreferenceCategory || view.persistenceWarning === null ? null : (
							<div className={styles['warning']} role='alert'>
								{view.persistenceWarning}
							</div>
						)}
						{view.category !== 'general' || view.localePersistenceWarning === null ? null : (
							<div className={styles['warning']} role='alert'>
								{view.localePersistenceWarning}
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
		</div>
	);
}
