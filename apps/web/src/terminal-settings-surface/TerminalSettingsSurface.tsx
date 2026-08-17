import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { Button, Input, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import { EmptyResults } from './components/empty-results/EmptyResults';
import { GhosttyMigrationSettings } from './components/ghostty-migration-settings/GhosttyMigrationSettings';
import { KeyboardShortcutSettings } from './components/keyboard-shortcut-settings/KeyboardShortcutSettings';
import { MacosTerminalInputSettings } from './components/macos-terminal-input-settings/MacosTerminalInputSettings';
import { RemoteDevelopmentSettings } from './components/remote-development-settings/RemoteDevelopmentSettings';
import { ShellIntegrationSettings } from './components/shell-integration-settings/ShellIntegrationSettings';
import { TerminalSettingsSections } from './components/terminal-settings-sections/TerminalSettingsSections';
import { useTerminalSettingsSurface } from './hooks/useTerminalSettingsSurface';
import type { TerminalSettingsController } from '../terminal-settings';
import type { KeyboardShortcutSettingsController } from '../keyboard-shortcuts';
import type { TerminalFeedbackSettingsController } from '../terminal-feedback-settings';
import type { MacosTerminalInputSettingsController } from '../macos-terminal-input-settings';
import styles from './TerminalSettingsSurface.module.css';

export interface TerminalSettingsSurfaceProps {
	readonly controller: TerminalSettingsController;
	readonly keyboardShortcutController: KeyboardShortcutSettingsController;
	readonly keyboardShortcutPlatform: KeyboardShortcutPlatform;
	readonly macosTerminalInputSettingsController?: MacosTerminalInputSettingsController | undefined;
	readonly onRequestClose: () => void;
	readonly requestBrowserAccess?: ((returnFocus: HTMLElement) => void) | undefined;
	readonly terminalFeedbackSettingsController?: TerminalFeedbackSettingsController | undefined;
}

export function TerminalSettingsSurface(props: TerminalSettingsSurfaceProps): React.ReactNode {
	const view = useTerminalSettingsSurface(props);
	if (!view.open) return null;
	const categoryItems = [
		{
			icon: 'terminal',
			id: 'terminal',
			label: view.messages.terminalCategory,
			onClick: view.onTerminalCategoryClick,
			visible: true,
		},
		{
			icon: 'action',
			id: 'keyboard-shortcuts',
			label: view.messages.keyboardShortcutsCategory,
			onClick: view.onKeyboardShortcutsCategoryClick,
			visible: true,
		},
		{
			icon: 'settings',
			id: 'macos-input',
			label: view.messages.macosInputCategory,
			onClick: view.onMacosInputCategoryClick,
			visible: view.showMacosInputCategory,
		},
		{
			icon: 'git-branch',
			id: 'remote-development',
			label: view.messages.remoteDevelopmentCategory,
			onClick: view.onRemoteDevelopmentCategoryClick,
			visible: true,
		},
		{
			icon: 'terminal',
			id: 'shell-integration',
			label: view.messages.shellIntegrationCategory,
			onClick: view.onShellIntegrationCategoryClick,
			visible: view.showShellIntegrationCategory,
		},
		{
			icon: 'clipboard',
			id: 'ghostty-migration',
			label: view.messages.ghosttyMigrationCategory,
			onClick: view.onGhosttyMigrationCategoryClick,
			visible: true,
		},
		{
			icon: 'screen-full',
			id: 'browser-access',
			label: view.messages.browserAccessCategory,
			onClick: view.onBrowserAccessCategoryClick,
			visible: view.showBrowserAccessCategory,
		},
	] as const;

	let mainContent: React.ReactNode;
	if (view.category === 'keyboard-shortcuts') {
		mainContent = (
			<KeyboardShortcutSettings
				controller={props.keyboardShortcutController}
				platform={props.keyboardShortcutPlatform}
				query={view.query}
			/>
		);
	} else if (view.category === 'ghostty-migration') {
		mainContent = view.ghosttyMigrationVisible ? (
			<GhosttyMigrationSettings />
		) : (
			<EmptyResults message={view.messages.noResults} />
		);
	} else if (
		view.category === 'macos-input' &&
		props.macosTerminalInputSettingsController !== undefined
	) {
		mainContent = (
			<MacosTerminalInputSettings
				controller={props.macosTerminalInputSettingsController}
				query={view.query}
			/>
		);
	} else if (view.category === 'browser-access') {
		mainContent = view.browserAccessVisible ? (
			<div className={styles['browserAccessAction']}>
				<Button onClick={view.onOpenBrowserAccess}>
					<ProductIcon name='screen-full' />
					{view.messages.openBrowserAccess}
				</Button>
			</div>
		) : (
			<EmptyResults message={view.messages.noResults} />
		);
	} else if (view.category === 'remote-development') {
		mainContent = view.remoteDevelopmentVisible ? (
			<RemoteDevelopmentSettings messages={view.messages} />
		) : (
			<EmptyResults message={view.messages.noResults} />
		);
	} else if (
		view.category === 'shell-integration' &&
		props.terminalFeedbackSettingsController !== undefined
	) {
		mainContent = (
			<ShellIntegrationSettings
				controller={props.terminalFeedbackSettingsController}
				query={view.query}
			/>
		);
	} else if (view.terminalResultCount === 0) {
		mainContent = <EmptyResults message={view.messages.noResults} />;
	} else {
		mainContent = <TerminalSettingsSections view={view} />;
	}
	let contentTitle: string;
	let contentDescription: string;
	switch (view.category) {
		case 'terminal':
			contentTitle = view.messages.terminalCategory;
			contentDescription = view.messages.description;
			break;
		case 'keyboard-shortcuts':
			contentTitle = view.messages.keyboardShortcutsTitle;
			contentDescription = view.messages.keyboardShortcutsDescription;
			break;
		case 'macos-input':
			contentTitle = view.messages.macosInputTitle;
			contentDescription = view.messages.macosInputDescription;
			break;
		case 'ghostty-migration':
			contentTitle = view.messages.ghosttyMigrationTitle;
			contentDescription = view.messages.ghosttyMigrationDescription;
			break;
		case 'browser-access':
			contentTitle = view.messages.browserAccessTitle;
			contentDescription = view.messages.browserAccessDescription;
			break;
		case 'remote-development':
			contentTitle = view.messages.remoteDevelopmentTitle;
			contentDescription = view.messages.remoteDevelopmentDescription;
			break;
		case 'shell-integration':
			contentTitle = view.messages.shellIntegrationTitle;
			contentDescription = view.messages.shellIntegrationDescription;
			break;
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
				<div className={styles['sidebarHeader']}>
					<Button
						aria-label={view.messages.back}
						className={styles['back']}
						size='icon-sm'
						title={view.messages.back}
						variant='ghost'
						onClick={view.onRequestClose}
					>
						<ProductIcon name='arrow-left' />
					</Button>
					<span className={styles['sidebarTitle']}>{view.messages.title}</span>
				</div>
				<div className={styles['search']}>
					<ProductIcon name='search' />
					<Input
						ref={view.inputReference}
						aria-label={view.messages.searchPlaceholder}
						className={styles['searchInput']}
						placeholder={view.messages.searchPlaceholder}
						value={view.query}
						onChange={view.onSearchChange}
					/>
				</div>
				<nav aria-label={view.messages.title} className={styles['categories']}>
					{categoryItems.map((item) =>
						item.visible ? (
							<button
								aria-current={view.category === item.id ? 'page' : undefined}
								className={styles['category']}
								data-selected={view.category === item.id}
								data-settings-category={item.id}
								key={item.id}
								type='button'
								onClick={item.onClick}
							>
								<span className={styles['categoryIcon']}>
									<ProductIcon name={item.icon} />
								</span>
								<span className={styles['categoryLabel']}>{item.label}</span>
							</button>
						) : null,
					)}
				</nav>
			</aside>

			<main className={styles['content']} data-settings-content={view.category}>
				<div className={styles['contentColumn']}>
					<header className={styles['header']}>
						<h1 className={styles['title']} id='terminal-settings-title'>
							{contentTitle}
						</h1>
						<p className={styles['description']}>{contentDescription}</p>
					</header>

					{view.category !== 'terminal' || view.persistenceWarning === null ? null : (
						<div className={styles['warning']} role='alert'>
							{view.persistenceWarning}
						</div>
					)}

					{mainContent}
				</div>
			</main>
		</div>
	);
}
