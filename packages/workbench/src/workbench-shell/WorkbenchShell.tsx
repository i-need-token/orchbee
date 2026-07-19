import { ResizableHandle, ResizablePanel, ResizablePanelGroup } from '@orchbee/ui/components';
import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import * as React from 'react';

import styles from './WorkbenchShell.module.css';

export interface WorkbenchShellProps {
	readonly sidebar: React.ReactNode;
	readonly toolbar: React.ReactNode;
	readonly tabs: React.ReactNode;
	readonly content: React.ReactNode;
	readonly status: React.ReactNode;
}

export function WorkbenchShell({
	sidebar,
	toolbar,
	tabs,
	content,
	status,
}: WorkbenchShellProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	return (
		<div className={styles['root']}>
			<div className={styles['layout']}>
				<ResizablePanelGroup orientation='horizontal'>
					{/* react-resizable-panels v4 interprets unitless numeric sizes as pixels. */}
					<ResizablePanel
						defaultSize='20rem'
						minSize='16rem'
						maxSize='24rem'
						groupResizeBehavior='preserve-pixel-size'
					>
						<aside
							className={styles['sidebar']}
							aria-label={intl.formatMessage(workbenchMessages.workbenchNavigationLabel)}
						>
							<header className={styles['sidebarHeader']}>
								<span className={styles['brandMark']} aria-hidden='true'>
									OB
								</span>
								<span className={styles['brand']}>Orchbee</span>
							</header>
							<div className={styles['sidebarBody']}>{sidebar}</div>
							<footer className={styles['sidebarFooter']}>{status}</footer>
						</aside>
					</ResizablePanel>
					<ResizableHandle className={styles['resizeHandle']} />
					<ResizablePanel minSize='40%'>
						<div className={styles['mainColumn']}>
							<header className={styles['terminalHeader']}>
								<div className={styles['tabs']}>{tabs}</div>
								<div className={styles['toolbar']}>{toolbar}</div>
							</header>
							<main className={styles['content']}>{content}</main>
						</div>
					</ResizablePanel>
				</ResizablePanelGroup>
			</div>
		</div>
	);
}
