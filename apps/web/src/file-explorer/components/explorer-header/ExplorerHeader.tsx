import { fileExplorerMessages, useOrchbeeIntl } from '@orchbee/i18n';
import {
	ProductIcon,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
	type ProductIconName,
} from '@orchbee/ui/components';
import type { ReactNode } from 'react';

import styles from './ExplorerHeader.module.css';

interface ExplorerHeaderProps {
	/** Whether a folder is expanded for Collapse Folders in Explorer to collapse. */
	readonly canCollapse: boolean;
	/** What VS Code titles the root: its folder's name, as the machine spells it. */
	readonly name: string;
	readonly onCollapse: () => void;
	readonly onRefresh: () => void;
	/** The root's whole path, which the title's tooltip shows. */
	readonly path: string;
}

interface ExplorerHeaderToolProps {
	readonly disabled: boolean;
	readonly icon: ProductIconName;
	readonly label: string;
	readonly onPress: () => void;
}

function ExplorerHeaderTool({
	disabled,
	icon,
	label,
	onPress,
}: ExplorerHeaderToolProps): ReactNode {
	return (
		<Tooltip>
			<TooltipTrigger
				render={
					<button
						aria-label={label}
						className={styles['tool']}
						disabled={disabled}
						type='button'
						onClick={onPress}
					/>
				}
			>
				<ProductIcon name={icon} size='section-action' />
			</TooltipTrigger>
			<TooltipContent side='bottom'>{label}</TooltipContent>
		</Tooltip>
	);
}

/**
 * The Explorer's section header, VS Code's pane title with the root's name, and its title Actions,
 * which show while the pointer or focus is in the Explorer.
 */
export function ExplorerHeader({
	canCollapse,
	name,
	onCollapse,
	onRefresh,
	path,
}: ExplorerHeaderProps): ReactNode {
	const intl = useOrchbeeIntl();
	return (
		<div className={styles['root']} data-slot='file-explorer-header'>
			<Tooltip>
				<TooltipTrigger
					render={
						<h2
							aria-label={intl.formatMessage(fileExplorerMessages.section, { name })}
							className={styles['title']}
						/>
					}
				>
					{name}
				</TooltipTrigger>
				<TooltipContent align='start' side='bottom'>
					{path}
				</TooltipContent>
			</Tooltip>
			<div className={styles['tools']}>
				<ExplorerHeaderTool
					disabled={false}
					icon='refresh'
					label={intl.formatMessage(fileExplorerMessages.refresh)}
					onPress={onRefresh}
				/>
				<ExplorerHeaderTool
					disabled={!canCollapse}
					icon='collapse-all'
					label={intl.formatMessage(fileExplorerMessages.collapse)}
					onPress={onCollapse}
				/>
			</div>
		</div>
	);
}
