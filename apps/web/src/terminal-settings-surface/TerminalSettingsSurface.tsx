import type { KeyboardShortcutPlatform } from '@orchbee/keyboard-shortcuts';
import { Button, Input, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import { EmptyResults } from './components/empty-results/EmptyResults';
import { GhosttyMigrationSettings } from './components/ghostty-migration-settings/GhosttyMigrationSettings';
import { KeyboardShortcutSettings } from './components/keyboard-shortcut-settings/KeyboardShortcutSettings';
import { RemoteDevelopmentSettings } from './components/remote-development-settings/RemoteDevelopmentSettings';
import { ShellIntegrationSettings } from './components/shell-integration-settings/ShellIntegrationSettings';
import { TerminalSettingsSections } from './components/terminal-settings-sections/TerminalSettingsSections';
import { useTerminalSettingsSurface } from './hooks/useTerminalSettingsSurface';
import type { TerminalSettingsController } from '../terminal-settings';
import type { KeyboardShortcutSettingsController } from '../keyboard-shortcuts';
import type { TerminalFeedbackSettingsController } from '../terminal-feedback-settings';
import styles from './TerminalSettingsSurface.module.css';

export interface TerminalSettingsSurfaceProps {
	readonly controller: TerminalSettingsController;
	readonly keyboardShortcutController: KeyboardShortcutSettingsController;
	readonly keyboardShortcutPlatform: KeyboardShortcutPlatform;
	readonly onRequestClose: () => void;
	readonly requestBrowserAccess?: ((returnFocus: HTMLElement) => void) | undefined;
	readonly terminalFeedbackSettingsController?: TerminalFeedbackSettingsController | undefined;
}

export function TerminalSettingsSurface(props: TerminalSettingsSurfaceProps): React.ReactNode {
	const view = useTerminalSettingsSurface(props);
	if (!view.open) return null;

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
	} else if (view.category === 'browser-access') {
		mainContent = view.browserAccessVisible ? (
			<section className={styles['browserAccessCard']}>
				<div>
					<h2 className={styles['cardTitle']}>{view.messages.browserAccessTitle}</h2>
					<p className={styles['cardDescription']}>{view.messages.browserAccessDescription}</p>
				</div>
				<Button onClick={view.onOpenBrowserAccess}>{view.messages.openBrowserAccess}</Button>
			</section>
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
			contentTitle = view.messages.title;
			contentDescription = view.messages.description;
			break;
		case 'keyboard-shortcuts':
			contentTitle = view.messages.keyboardShortcutsTitle;
			contentDescription = view.messages.keyboardShortcutsDescription;
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
				<Button className={styles['back']} size='sm' variant='ghost' onClick={view.onRequestClose}>
					<ProductIcon name='arrow-left' />
					{view.messages.back}
				</Button>
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
					<button
						aria-current={view.category === 'terminal' ? 'page' : undefined}
						className={styles['category']}
						data-selected={view.category === 'terminal'}
						data-settings-category='terminal'
						type='button'
						onClick={view.onTerminalCategoryClick}
					>
						<ProductIcon name='terminal' />
						{view.messages.terminalCategory}
					</button>
					<button
						aria-current={view.category === 'keyboard-shortcuts' ? 'page' : undefined}
						className={styles['category']}
						data-selected={view.category === 'keyboard-shortcuts'}
						data-settings-category='keyboard-shortcuts'
						type='button'
						onClick={view.onKeyboardShortcutsCategoryClick}
					>
						<ProductIcon name='action' />
						{view.messages.keyboardShortcutsCategory}
					</button>
					<button
						aria-current={view.category === 'remote-development' ? 'page' : undefined}
						className={styles['category']}
						data-selected={view.category === 'remote-development'}
						data-settings-category='remote-development'
						type='button'
						onClick={view.onRemoteDevelopmentCategoryClick}
					>
						<ProductIcon name='terminal' />
						{view.messages.remoteDevelopmentCategory}
					</button>
					{view.showShellIntegrationCategory ? (
						<button
							aria-current={view.category === 'shell-integration' ? 'page' : undefined}
							className={styles['category']}
							data-selected={view.category === 'shell-integration'}
							data-settings-category='shell-integration'
							type='button'
							onClick={view.onShellIntegrationCategoryClick}
						>
							<ProductIcon name='terminal' />
							{view.messages.shellIntegrationCategory}
						</button>
					) : null}
					<button
						aria-current={view.category === 'ghostty-migration' ? 'page' : undefined}
						className={styles['category']}
						data-selected={view.category === 'ghostty-migration'}
						data-settings-category='ghostty-migration'
						type='button'
						onClick={view.onGhosttyMigrationCategoryClick}
					>
						<ProductIcon name='clipboard' />
						{view.messages.ghosttyMigrationCategory}
					</button>
					{view.showBrowserAccessCategory ? (
						<button
							aria-current={view.category === 'browser-access' ? 'page' : undefined}
							className={styles['category']}
							data-selected={view.category === 'browser-access'}
							data-settings-category='browser-access'
							type='button'
							onClick={view.onBrowserAccessCategoryClick}
						>
							<ProductIcon name='screen-full' />
							{view.messages.browserAccessCategory}
						</button>
					) : null}
				</nav>
			</aside>

			<main className={styles['content']}>
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
