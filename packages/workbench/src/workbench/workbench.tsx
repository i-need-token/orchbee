import { useOrchbeeIntl, workbenchMessages } from '@orchbee/i18n';
import {
	Button,
	DropdownMenu,
	DropdownMenuContent,
	DropdownMenuItem,
	DropdownMenuSeparator,
	DropdownMenuTrigger,
	ResizableHandle,
	ResizablePanel,
	ResizablePanelGroup,
	Tabs,
	TabsList,
	TabsTrigger,
} from '@orchbee/ui/components';
import * as React from 'react';

import type { WorkbenchSessionService } from '../sessions';
import { TerminalPane } from '../terminal-pane';
import { WorkbenchShell } from '../workbench-shell';
import { useWorkbench } from './hooks/use-workbench';
import styles from './workbench.module.css';

export interface WorkbenchProps {
	readonly sessions: WorkbenchSessionService;
}

export function Workbench({ sessions: sessionService }: WorkbenchProps): React.ReactNode {
	const intl = useOrchbeeIntl();
	const workbench = useWorkbench(sessionService);

	const sidebar = (
		<div className={styles['sidebarContent']}>
			<Button
				className={styles['newSessionButton']}
				variant='ghost'
				size='sm'
				onClick={workbench.createSession}
			>
				<span aria-hidden='true'>＋</span>
				{intl.formatMessage(workbenchMessages.newTerminal)}
			</Button>
			<section className={styles['sidebarSection']} aria-labelledby='sessions-heading'>
				<div className={styles['sectionHeadingRow']}>
					<div id='sessions-heading' className={styles['sidebarHeading']}>
						{intl.formatMessage(workbenchMessages.sessionsHeading)}
					</div>
					<span className={styles['sectionCount']}>{workbench.sessions.length}</span>
				</div>
				<div className={styles['sessionList']}>
					{workbench.sessions.map((session) => (
						<Button
							key={session.id}
							className={
								session.id === workbench.activeSessionId
									? styles['selectedSessionButton']
									: styles['sessionButton']
							}
							variant='ghost'
							size='sm'
							onClick={() => workbench.selectSession(session.id)}
						>
							<span
								className={styles['sessionStatusIndicator']}
								data-status={session.status}
								aria-hidden='true'
							/>
							<span className={styles['sessionTitle']}>{session.title}</span>
						</Button>
					))}
				</div>
			</section>
		</div>
	);

	const toolbar = (
		<DropdownMenu>
			<DropdownMenuTrigger
				render={
					<Button
						variant='ghost'
						size='icon-xs'
						aria-label={intl.formatMessage(workbenchMessages.terminalActionsLabel)}
					/>
				}
			>
				•••
			</DropdownMenuTrigger>
			<DropdownMenuContent align='end'>
				<DropdownMenuItem
					disabled={workbench.activeSession === undefined}
					onClick={() => workbench.split('horizontal')}
				>
					{intl.formatMessage(workbenchMessages.splitRight)}
				</DropdownMenuItem>
				<DropdownMenuItem
					disabled={workbench.activeSession === undefined}
					onClick={() => workbench.split('vertical')}
				>
					{intl.formatMessage(workbenchMessages.splitDown)}
				</DropdownMenuItem>
				{workbench.splitSession !== undefined && (
					<DropdownMenuItem onClick={workbench.closeSplit}>
						{intl.formatMessage(workbenchMessages.closeSplit)}
					</DropdownMenuItem>
				)}
				<DropdownMenuSeparator />
				<DropdownMenuItem
					variant='destructive'
					disabled={workbench.activeSession === undefined}
					onClick={workbench.closeActiveSession}
				>
					{intl.formatMessage(workbenchMessages.closeTerminal)}
				</DropdownMenuItem>
			</DropdownMenuContent>
		</DropdownMenu>
	);

	const tabs =
		workbench.activeSessionId === undefined ? null : (
			<Tabs
				className={styles['tabs']}
				value={workbench.activeSessionId}
				onValueChange={(value) => {
					if (typeof value === 'string') workbench.selectSession(value);
				}}
			>
				<TabsList className={styles['tabList']} variant='line'>
					{workbench.sessions.map((session) => (
						<TabsTrigger key={session.id} className={styles['tab']} value={session.id}>
							{session.title}
						</TabsTrigger>
					))}
				</TabsList>
			</Tabs>
		);

	let content: React.ReactNode = (
		<div className={styles['emptyState']}>
			<div className={styles['emptyTitle']}>
				{intl.formatMessage(workbenchMessages.emptySessionsTitle)}
			</div>
			<div className={styles['emptyDescription']}>
				{intl.formatMessage(workbenchMessages.emptySessionsDescription)}
			</div>
		</div>
	);
	if (workbench.activeSession !== undefined) {
		content =
			workbench.splitSession === undefined ? (
				<TerminalPane
					session={workbench.activeSession}
					active
					showHeader={false}
					onFocus={() => workbench.focusPane('primary')}
				/>
			) : (
				<ResizablePanelGroup
					key={workbench.splitOrientation}
					orientation={workbench.splitOrientation}
				>
					<ResizablePanel defaultSize='50%' minSize='20%'>
						<TerminalPane
							session={workbench.activeSession}
							active={workbench.focusedPane === 'primary'}
							onFocus={() => workbench.focusPane('primary')}
						/>
					</ResizablePanel>
					<ResizableHandle className={styles['resizeHandle']} withHandle />
					<ResizablePanel defaultSize='50%' minSize='20%'>
						<TerminalPane
							session={workbench.splitSession}
							active={workbench.focusedPane === 'secondary'}
							onFocus={() => workbench.focusPane('secondary')}
						/>
					</ResizablePanel>
				</ResizablePanelGroup>
			);
	}

	return (
		<WorkbenchShell
			sidebar={sidebar}
			toolbar={toolbar}
			tabs={tabs}
			content={content}
			status={
				<div className={styles['runtimeStatus']}>
					<span className={styles['runtimeStatusIndicator']} aria-hidden='true' />
					<span>{intl.formatMessage(workbenchMessages.localRuntime)}</span>
					<span className={styles['runtimeSessionCount']}>
						{intl.formatMessage(workbenchMessages.sessionCount, {
							sessionCount: workbench.sessions.length,
						})}
					</span>
				</div>
			}
		/>
	);
}
