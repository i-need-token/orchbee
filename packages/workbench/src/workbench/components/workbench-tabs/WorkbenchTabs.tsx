import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import {
	Button,
	ProductIcon,
	Tabs,
	TabsBar,
	TabsContent,
	TabsDivider,
	TabsItem,
	TabsList,
	TabsTrigger,
	Tooltip,
	TooltipContent,
	TooltipTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchTab } from '../../types/workbench-tab';
import { useWorkbenchTabDrag } from './hooks/useWorkbenchTabDrag';
import styles from './WorkbenchTabs.module.css';

interface WorkbenchTabsProps {
	readonly activeTabId: string | undefined;
	readonly ariaLabel: string;
	readonly onActiveTabChange: ((tabId: string) => void) | undefined;
	readonly onTabClose: ((tabId: string) => void) | undefined;
	readonly onTabCreate: (() => void) | undefined;
	readonly onTabMove: ((tabId: string, targetIndex: number) => void) | undefined;
	readonly reserveLeadingControlSpace: boolean;
	readonly tabs: readonly WorkbenchTab[];
}

export function WorkbenchTabs({
	activeTabId,
	ariaLabel,
	onActiveTabChange,
	onTabClose,
	onTabCreate,
	onTabMove,
	reserveLeadingControlSpace,
	tabs,
}: WorkbenchTabsProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const tabDrag = useWorkbenchTabDrag(tabs, onTabMove, styles['dragPreview'] ?? '');
	if (tabs.length === 0 && !reserveLeadingControlSpace && onTabCreate === undefined) return null;

	return (
		<Tabs
			defaultValue={activeTabId === undefined ? tabs[0]?.id : undefined}
			value={activeTabId}
			variant='workspace'
			onValueChange={(value) => {
				if (typeof value === 'string') onActiveTabChange?.(value);
			}}
		>
			<TabsBar>
				{reserveLeadingControlSpace && (
					<div
						aria-hidden='true'
						className={styles['leadingControlSpace']}
						data-slot='workbench-sidebar-toggle-space'
					/>
				)}
				<div className={styles['tabCluster']}>
					{tabs.length > 0 && (
						<TabsList
							aria-label={ariaLabel}
							className={styles['tabList']}
							data-hover-suppressed={tabDrag.hoverSuppressed ? true : undefined}
							variant='workspace'
							onPointerMove={tabDrag.onPointerMove}
						>
							<TabsDivider
								className={styles['divider']}
								data-drop-indicator={tabDrag.dropBoundaryIndex === 0 ? true : undefined}
								data-edge='start'
							/>
							{tabs.map((tab, tabIndex) => {
								const closeLabel = intl.formatMessage(workbenchMessages.closeTab, {
									tabLabel: tab.label,
								});
								const dragProperties = tabDrag.getTabDragProperties(tab.id);

								return (
									<React.Fragment key={tab.id}>
										<TabsItem
											className={styles['tabItem']}
											data-dragging={dragProperties['data-dragging']}
											data-workbench-tab-item-id={tab.id}
											draggable={dragProperties.draggable}
											onDragEnd={dragProperties.onDragEnd}
											onDragOver={dragProperties.onDragOver}
											onDragStart={dragProperties.onDragStart}
											onDrop={dragProperties.onDrop}
										>
											<TabsTrigger className={styles['trigger']} value={tab.id}>
												<span className={styles['label']}>{tab.label}</span>
											</TabsTrigger>
											{onTabClose !== undefined && (
												<Tooltip>
													<TooltipTrigger
														render={
															<Button
																aria-label={closeLabel}
																className={styles['closeButton']}
																data-workbench-tab-action='close'
																data-workbench-close-tab-id={tab.id}
																size='icon-xs'
																type='button'
																variant='ghost'
																onClick={() => {
																	onTabClose(tab.id);
																}}
															/>
														}
													>
														<ProductIcon name='close' />
													</TooltipTrigger>
													<TooltipContent side='bottom'>{closeLabel}</TooltipContent>
												</Tooltip>
											)}
										</TabsItem>
										<TabsDivider
											className={styles['divider']}
											data-drop-indicator={
												tabDrag.dropBoundaryIndex === tabIndex + 1 ? true : undefined
											}
										/>
									</React.Fragment>
								);
							})}
						</TabsList>
					)}
					{onTabCreate !== undefined && (
						<Tooltip>
							<TooltipTrigger
								render={
									<Button
										aria-label={intl.formatMessage(workbenchMessages.addTab)}
										className={styles['addButton']}
										data-slot='workbench-tab-add'
										size='icon-xs'
										type='button'
										variant='ghost'
										onClick={onTabCreate}
									/>
								}
							>
								<ProductIcon name='add' />
							</TooltipTrigger>
							<TooltipContent side='bottom'>
								{intl.formatMessage(workbenchMessages.addTab)}
							</TooltipContent>
						</Tooltip>
					)}
				</div>
			</TabsBar>
			{tabs.map((tab) => (
				<TabsContent key={tab.id} data-workbench-tab-id={tab.id} value={tab.id} />
			))}
		</Tabs>
	);
}
