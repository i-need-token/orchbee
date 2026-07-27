import { Button, Kbd, ProductIcon } from '@orchbee/ui/components';
import * as React from 'react';

import styles from './WorkbenchSidebar.module.css';

interface WorkbenchSidebarProps {
	readonly label: string;
	readonly newTerminalLabel: string;
	readonly newTerminalShortcutId: string;
	readonly newTerminalShortcutLabel: string;
	readonly onNewTerminal?: (() => void) | undefined;
	readonly onSearch: (returnFocus: HTMLElement) => void;
	readonly onSettings?: (() => void) | undefined;
	readonly projectLabel: string;
	readonly searchLabel: string;
	readonly searchShortcutId: string;
	readonly searchShortcutLabel: string;
	readonly settingsLabel: string;
	readonly settingsShortcutId: string;
	readonly settingsShortcutLabel: string;
}

export function WorkbenchSidebar({
	label,
	newTerminalLabel,
	newTerminalShortcutId,
	newTerminalShortcutLabel,
	onNewTerminal,
	onSearch,
	onSettings,
	projectLabel,
	searchLabel,
	searchShortcutId,
	searchShortcutLabel,
	settingsLabel,
	settingsShortcutId,
	settingsShortcutLabel,
}: WorkbenchSidebarProps): React.ReactNode {
	return (
		<aside aria-label={label} className={styles['root']} data-slot='workbench-sidebar'>
			<ul className={styles['primaryList']}>
				<li>
					<Button
						aria-disabled={onNewTerminal === undefined}
						className={styles['primaryItem']}
						data-shortcut-action={newTerminalShortcutId}
						data-slot='workbench-sidebar-new-terminal'
						size='sm'
						variant='ghost'
						onClick={onNewTerminal}
					>
						<span className={styles['icon']}>
							<ProductIcon name='terminal' />
						</span>
						<span className={styles['label']}>{newTerminalLabel}</span>
						<Kbd className={styles['shortcut']}>{newTerminalShortcutLabel}</Kbd>
					</Button>
				</li>
				<li>
					<Button
						className={styles['primaryItem']}
						data-shortcut-action={searchShortcutId}
						data-slot='workbench-sidebar-search'
						size='sm'
						variant='ghost'
						onClick={(event) => onSearch(event.currentTarget)}
					>
						<span className={styles['icon']}>
							<ProductIcon name='search' />
						</span>
						<span className={styles['label']}>{searchLabel}</span>
						<Kbd className={styles['shortcut']}>{searchShortcutLabel}</Kbd>
					</Button>
				</li>
			</ul>
			<section className={styles['projectSection']}>
				<h2 className={styles['sectionHeading']}>{projectLabel}</h2>
			</section>
			<div className={styles['footer']}>
				<Button
					aria-disabled={onSettings === undefined}
					className={styles['primaryItem']}
					data-shortcut-action={settingsShortcutId}
					data-slot='workbench-sidebar-settings'
					size='sm'
					variant='ghost'
					onClick={onSettings}
				>
					<span className={styles['icon']}>
						<ProductIcon name='settings' />
					</span>
					<span className={styles['label']}>{settingsLabel}</span>
					<Kbd className={styles['shortcut']}>{settingsShortcutLabel}</Kbd>
				</Button>
			</div>
		</aside>
	);
}
