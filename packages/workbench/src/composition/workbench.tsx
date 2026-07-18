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
import { useStore } from 'zustand';

import type { WorkbenchSession, WorkbenchSessionService } from '../sessions';
import {
	createWorkbenchLayoutStore,
	WorkbenchShell,
	type WorkbenchLayoutStore,
	type WorkbenchSplitOrientation,
} from '../shell';
import { TerminalPane } from '../terminals';
import styles from './workbench.module.css';

export interface WorkbenchProps {
	readonly sessions: WorkbenchSessionService;
}

function findSession(
	sessions: readonly WorkbenchSession[],
	sessionId: string | undefined,
): WorkbenchSession | undefined {
	return sessions.find((session) => session.id === sessionId);
}

function findSplitCandidate(
	sessions: readonly WorkbenchSession[],
	activeSessionId: string | undefined,
): string | undefined {
	return sessions.find((session) => session.id !== activeSessionId)?.id;
}

function splitWorkbench(
	store: WorkbenchLayoutStore,
	sessionService: WorkbenchSessionService,
	sessions: readonly WorkbenchSession[],
	orientation: WorkbenchSplitOrientation,
): void {
	const state = store.getState();
	const splitSessionId = findSplitCandidate(sessions, state.activeSessionId);
	store.getState().splitWithSession(splitSessionId ?? sessionService.createSession(), orientation);
}

export function Workbench({ sessions: sessionService }: WorkbenchProps): React.ReactNode {
	const snapshot = React.useSyncExternalStore(
		(listener) => sessionService.subscribe(listener),
		() => sessionService.getSnapshot(),
		() => sessionService.getSnapshot(),
	);
	const [layoutStore] = React.useState(() => createWorkbenchLayoutStore(snapshot.sessions[0]?.id));
	const activeSessionId = useStore(layoutStore, (state) => state.activeSessionId);
	const splitSessionId = useStore(layoutStore, (state) => state.splitSessionId);
	const splitOrientation = useStore(layoutStore, (state) => state.splitOrientation);
	const focusedPane = useStore(layoutStore, (state) => state.focusedPane);
	const activeSession = findSession(snapshot.sessions, activeSessionId);
	const splitSession = findSession(snapshot.sessions, splitSessionId);

	React.useEffect(() => {
		layoutStore.getState().reconcileSessions(snapshot.sessions.map((session) => session.id));
	}, [layoutStore, snapshot]);

	const sidebar = (
		<div className={styles['sidebarContent']}>
			<Button
				className={styles['newSessionButton']}
				variant='ghost'
				size='sm'
				onClick={() => {
					const sessionId = sessionService.createSession();
					layoutStore.getState().selectSession(sessionId);
				}}
			>
				<span aria-hidden='true'>＋</span>
				New terminal
			</Button>
			<section className={styles['sidebarSection']} aria-labelledby='sessions-heading'>
				<div className={styles['sectionHeadingRow']}>
					<div id='sessions-heading' className={styles['sidebarHeading']}>
						Sessions
					</div>
					<span className={styles['sectionCount']}>{snapshot.sessions.length}</span>
				</div>
				<div className={styles['sessionList']}>
					{snapshot.sessions.map((session) => (
						<Button
							key={session.id}
							className={
								session.id === activeSessionId
									? styles['selectedSessionButton']
									: styles['sessionButton']
							}
							variant='ghost'
							size='sm'
							onClick={() => layoutStore.getState().selectSession(session.id)}
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
				render={<Button variant='ghost' size='icon-xs' aria-label='Terminal actions' />}
			>
				•••
			</DropdownMenuTrigger>
			<DropdownMenuContent align='end'>
				<DropdownMenuItem
					disabled={activeSession === undefined}
					onClick={() =>
						splitWorkbench(layoutStore, sessionService, snapshot.sessions, 'horizontal')
					}
				>
					Split right
				</DropdownMenuItem>
				<DropdownMenuItem
					disabled={activeSession === undefined}
					onClick={() => splitWorkbench(layoutStore, sessionService, snapshot.sessions, 'vertical')}
				>
					Split down
				</DropdownMenuItem>
				{splitSession !== undefined && (
					<DropdownMenuItem onClick={() => layoutStore.getState().closeSplit()}>
						Close split
					</DropdownMenuItem>
				)}
				<DropdownMenuSeparator />
				<DropdownMenuItem
					variant='destructive'
					disabled={activeSession === undefined}
					onClick={() => {
						if (activeSession !== undefined) sessionService.closeSession(activeSession.id);
					}}
				>
					Close terminal
				</DropdownMenuItem>
			</DropdownMenuContent>
		</DropdownMenu>
	);

	const tabs =
		activeSessionId === undefined ? null : (
			<Tabs
				className={styles['tabs']}
				value={activeSessionId}
				onValueChange={(value) => {
					if (typeof value === 'string') layoutStore.getState().selectSession(value);
				}}
			>
				<TabsList className={styles['tabList']} variant='line'>
					{snapshot.sessions.map((session) => (
						<TabsTrigger key={session.id} className={styles['tab']} value={session.id}>
							{session.title}
						</TabsTrigger>
					))}
				</TabsList>
			</Tabs>
		);

	let content: React.ReactNode = (
		<div className={styles['emptyState']}>
			<div className={styles['emptyTitle']}>No sessions are open</div>
			<div className={styles['emptyDescription']}>
				Create an in-memory session to start working.
			</div>
		</div>
	);
	if (activeSession !== undefined) {
		content =
			splitSession === undefined ? (
				<TerminalPane
					session={activeSession}
					active
					showHeader={false}
					onFocus={() => layoutStore.getState().focusPane('primary')}
				/>
			) : (
				<ResizablePanelGroup key={splitOrientation} orientation={splitOrientation}>
					<ResizablePanel defaultSize='50%' minSize='20%'>
						<TerminalPane
							session={activeSession}
							active={focusedPane === 'primary'}
							onFocus={() => layoutStore.getState().focusPane('primary')}
						/>
					</ResizablePanel>
					<ResizableHandle className={styles['resizeHandle']} withHandle />
					<ResizablePanel defaultSize='50%' minSize='20%'>
						<TerminalPane
							session={splitSession}
							active={focusedPane === 'secondary'}
							onFocus={() => layoutStore.getState().focusPane('secondary')}
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
					<span>Local runtime</span>
					<span className={styles['runtimeSessionCount']}>
						{snapshot.sessions.length} {snapshot.sessions.length === 1 ? 'session' : 'sessions'}
					</span>
				</div>
			}
		/>
	);
}
