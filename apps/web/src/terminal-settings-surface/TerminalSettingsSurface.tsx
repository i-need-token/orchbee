import {
	keyboardShortcutMessages,
	terminalSettingsMessages,
	useOrchbeeIntl,
	type MessageDescriptor,
} from '@orchbee/i18n';
import { Input, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import {
	type KeyboardShortcutSettingsController,
	useKeyboardShortcutSettings,
} from '../keyboard-shortcuts';
import type { TerminalDefaultSettingsControl } from '../terminal-default-settings-control';
import { AgentSettings } from './components/agent-settings/AgentSettings';
import { AppearanceSettings } from './components/appearance-settings/AppearanceSettings';
import { ConfigFileRow } from './components/config-file-row/ConfigFileRow';
import { GeneralSettings } from './components/general-settings/GeneralSettings';
import { KeyboardShortcutSettings } from './components/keyboard-shortcut-settings/KeyboardShortcutSettings';
import { NotificationPermissionBanner } from './components/notification-permission-banner/NotificationPermissionBanner';
import { NotificationSettings } from './components/notification-settings/NotificationSettings';
import { SettingsBanner } from './components/settings-banner/SettingsBanner';
import { SettingsFileStatus } from './components/settings-file-status/SettingsFileStatus';
import { TerminalBehaviorSettings } from './components/terminal-behavior-settings/TerminalBehaviorSettings';
import { ThemeSettings } from './components/theme-settings/ThemeSettings';
import { TERMINAL_SETTINGS_CATEGORIES } from './constants/terminal-settings-categories';
import { useUserSettings } from './hooks/useUserSettings';
import { useTerminalSettingsSurface } from './hooks/useTerminalSettingsSurface';
import type {
	TerminalSettingsCategory,
	TerminalSettingsSurfaceProps,
} from './types/terminal-settings-surface';
import styles from './TerminalSettingsSurface.module.css';

export type { TerminalSettingsSurfaceProps } from './types/terminal-settings-surface';

/**
 * Settings as a full page over the workbench content: the settings navigation takes the app
 * sidebar's place, Back and Escape return to where the user was, and every change applies at once.
 */
export function TerminalSettingsSurface(props: TerminalSettingsSurfaceProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const page = useTerminalSettingsSurface(props);
	const userSettings = props.controller.userSettings;
	const filePath = useUserSettings(userSettings).snapshot?.filePath ?? null;
	if (!page.open) return null;
	const searchLabel = intl.formatMessage(terminalSettingsMessages.searchPlaceholder);
	const currentCategory = TERMINAL_SETTINGS_CATEGORIES.find(
		(definition) => definition.id === page.category,
	);
	const banner = (id: string): BannerVisibility => ({
		onDismiss: () => page.dismissBanner(id),
		visible: !page.searching && !page.isBannerDismissed(id),
	});

	return (
		<div
			className={styles['root']}
			data-slot='terminal-settings-surface'
			onKeyDown={page.onSurfaceKeyDown}
		>
			<div aria-hidden='true' className={styles['windowDragRegion']} />
			<nav
				aria-label={intl.formatMessage(terminalSettingsMessages.title)}
				className={styles['sidebar']}
			>
				<button
					className={styles['back']}
					data-settings-close
					type='button'
					onClick={props.onRequestClose}
				>
					<ProductIcon name='arrow-left' />
					<span className={styles['navLabel']}>
						{intl.formatMessage(terminalSettingsMessages.back)}
					</span>
				</button>
				<label className={styles['search']}>
					<ProductIcon name='search' />
					<Input
						ref={page.searchReference}
						aria-label={searchLabel}
						autoComplete='off'
						className={styles['searchInput']}
						placeholder={searchLabel}
						spellCheck={false}
						type='search'
						value={page.query}
						onChange={(event) => page.onQueryChange(event.currentTarget.value)}
					/>
				</label>
				<ul className={styles['categories']}>
					{page.categories.map((definition) => {
						const selected = !page.searching && definition.id === page.category;
						return (
							<li key={definition.id}>
								<button
									aria-current={selected ? 'page' : undefined}
									className={styles['category']}
									data-selected={selected}
									data-settings-category={definition.id}
									type='button'
									onClick={() => page.onCategorySelect(definition.id)}
								>
									<ProductIcon name={definition.icon} />
									<span className={styles['navLabel']}>{intl.formatMessage(definition.label)}</span>
								</button>
							</li>
						);
					})}
				</ul>
			</nav>
			<main
				ref={page.contentReference}
				className={styles['content']}
				data-settings-content={page.searching ? 'search' : page.category}
			>
				<div className={styles['column']}>
					{page.searching ? null : (
						<div className={styles['banners']}>
							<SettingsFileStatus
								control={userSettings}
								dismissed={page.isBannerDismissed('settings-file')}
								onDismiss={banner('settings-file').onDismiss}
							/>
							{page.category === 'terminal' &&
							props.terminalDefaultSettingsControl !== undefined ? (
								<TerminalDefaultsBanner
									control={props.terminalDefaultSettingsControl}
									visibility={banner('terminal-defaults')}
								/>
							) : null}
							{page.category === 'keyboard' ? (
								<KeyboardPersistenceBanner
									controller={props.keyboardShortcutController}
									visibility={banner('keyboard-persistence')}
								/>
							) : null}
							{page.category === 'notifications' &&
							props.terminalFeedbackSettingsController !== undefined &&
							banner('notification-permission').visible ? (
								<NotificationPermissionBanner
									controller={props.terminalFeedbackSettingsController}
									onDismiss={banner('notification-permission').onDismiss}
								/>
							) : null}
						</div>
					)}
					<h1 className={styles['title']} id='terminal-settings-title' tabIndex={-1}>
						{page.searching || currentCategory === undefined
							? intl.formatMessage(terminalSettingsMessages.pageSearchResults)
							: intl.formatMessage(currentCategory.label)}
					</h1>
					{page.searching ? (
						<div className={styles['searchPage']} data-slot='settings-search-results'>
							{page.categories.map((definition) => (
								<section
									aria-label={intl.formatMessage(definition.label)}
									className={styles['searchCategory']}
									key={definition.id}
								>
									<h2 className={styles['searchCategoryTitle']}>
										{intl.formatMessage(definition.label)}
									</h2>
									<CategoryContent category={definition.id} props={props} query={page.query} />
								</section>
							))}
							<section className={styles['searchCategory']}>
								<ConfigFileRow filePath={filePath} query={page.query} />
							</section>
							<p className={styles['noMatches']} role='status'>
								{intl.formatMessage(terminalSettingsMessages.pageNoMatches)}
							</p>
						</div>
					) : (
						<div className={styles['page']}>
							<CategoryContent category={page.category} props={props} query='' />
							{currentCategory?.configFile === true ? (
								<ConfigFileRow filePath={filePath} query='' />
							) : null}
						</div>
					)}
				</div>
			</main>
		</div>
	);
}

interface CategoryContentProps {
	readonly category: TerminalSettingsCategory;
	readonly props: TerminalSettingsSurfaceProps;
	readonly query: string;
}

function CategoryContent({ category, props, query }: CategoryContentProps): React.ReactNode {
	const userSettings = props.controller.userSettings;
	switch (category) {
		case 'general':
			return (
				<GeneralSettings
					applicationLocaleController={props.applicationLocaleController}
					query={query}
					userSettings={userSettings}
				/>
			);
		case 'appearance':
			return <AppearanceSettings controller={props.controller} query={query} />;
		case 'themes':
			return <ThemeSettings controller={props.controller} query={query} />;
		case 'terminal':
			return (
				<TerminalBehaviorSettings
					defaultSettingsControl={props.terminalDefaultSettingsControl}
					interactionController={props.terminalInteractionSettingsController}
					launchProfileClient={props.terminalLaunchProfileClient}
					macosInputController={props.macosTerminalInputSettingsController}
					query={query}
					userSettings={userSettings}
				/>
			);
		case 'notifications':
			return props.terminalFeedbackSettingsController === undefined ? null : (
				<NotificationSettings controller={props.terminalFeedbackSettingsController} query={query} />
			);
		case 'keyboard':
			return (
				<KeyboardShortcutSettings
					controller={props.keyboardShortcutController}
					platform={props.keyboardShortcutPlatform}
					query={query}
					takenByConfiguredCommands={props.keyboardShortcutsTakenByConfiguredCommands}
				/>
			);
		case 'agents':
			return (
				<AgentSettings
					integrationControl={props.agentIntegrationManagementControl}
					query={query}
					userSettings={userSettings}
				/>
			);
	}
}

interface BannerVisibility {
	readonly onDismiss: () => void;
	readonly visible: boolean;
}

interface BannerWithVisibility {
	readonly visibility: BannerVisibility;
}

const KEYBOARD_PERSISTENCE_WARNINGS = {
	'invalid-record': keyboardShortcutMessages.persistenceInvalidWarning,
	'read-failed': keyboardShortcutMessages.persistenceReadWarning,
	'write-failed': keyboardShortcutMessages.persistenceWriteWarning,
} as const satisfies Readonly<Record<string, MessageDescriptor>>;

function PersistenceBanner({
	descriptor,
	visibility,
}: {
	readonly descriptor: MessageDescriptor | null;
	readonly visibility: BannerVisibility;
}): React.ReactNode {
	const intl = useOrchbeeIntl();
	if (!visibility.visible || descriptor === null) return null;
	return <SettingsBanner title={intl.formatMessage(descriptor)} onDismiss={visibility.onDismiss} />;
}

function KeyboardPersistenceBanner({
	controller,
	visibility,
}: BannerWithVisibility & {
	readonly controller: KeyboardShortcutSettingsController;
}): React.ReactNode {
	const { persistenceIssue } = useKeyboardShortcutSettings(controller);
	return (
		<PersistenceBanner
			descriptor={
				persistenceIssue === null ? null : KEYBOARD_PERSISTENCE_WARNINGS[persistenceIssue]
			}
			visibility={visibility}
		/>
	);
}

/** Save failures show in the shared banner; only a missing defaults snapshot is reported here. */
function TerminalDefaultsBanner({
	control,
	visibility,
}: BannerWithVisibility & { readonly control: TerminalDefaultSettingsControl }): React.ReactNode {
	const snapshot = React.useSyncExternalStore(
		control.subscribe,
		control.getSnapshot,
		control.getSnapshot,
	);
	return (
		<PersistenceBanner
			descriptor={
				snapshot.failureCode === 'control-unavailable'
					? terminalSettingsMessages.scrollbackBytesControlUnavailableWarning
					: null
			}
			visibility={visibility}
		/>
	);
}
