import type { DesktopBackgroundStatusSummary } from '@orchbee/desktop-client-protocol';

import {
	resolveWorkbenchAgentDisplayState,
	type WorkbenchAgentSource,
	type WorkbenchProjectWorkspaceRuntime,
} from '../../workbench';
import type { ServerWorkbenchSession } from '../../workbench-client';
import type { DesktopBackgroundStatusClient } from './DesktopBackgroundStatusClient';

interface BindDesktopBackgroundStatusOptions {
	readonly agentObservationSource: WorkbenchAgentSource;
	readonly client: Pick<DesktopBackgroundStatusClient, 'report'>;
	readonly projectWorkspaceRuntime: Pick<
		WorkbenchProjectWorkspaceRuntime,
		'countRunningTerminals' | 'subscribe'
	>;
	readonly session: Pick<ServerWorkbenchSession, 'onEnded'>;
}

const ENDED_SUMMARY: DesktopBackgroundStatusSummary = {
	attention: 0,
	doneUnseen: 0,
	failed: 0,
	runningTerminals: 0,
	workingAgents: 0,
};

/**
 * Reports the window's terminals and Agents to Main whenever they change, the changes of one task
 * merged into one report. A Session that ended leaves nothing running here, so it reports none.
 * Returns what stops reporting.
 */
export function bindDesktopBackgroundStatus({
	agentObservationSource,
	client,
	projectWorkspaceRuntime,
	session,
}: BindDesktopBackgroundStatusOptions): () => void {
	let scheduled = false;
	let stopped = false;
	const report = (): void => {
		scheduled = false;
		if (stopped) return;
		client.report(
			summarize(
				projectWorkspaceRuntime.countRunningTerminals(),
				agentObservationSource.getSnapshot().presentations.values(),
			),
		);
	};
	const schedule = (): void => {
		if (scheduled || stopped) return;
		scheduled = true;
		queueMicrotask(report);
	};
	let stopPublication: (() => void) | undefined;
	let stopAgents: (() => void) | undefined;
	const stop = (): void => {
		if (stopped) return;
		stopped = true;
		stopPublication?.();
		stopAgents?.();
		stopEnd();
	};
	// A Session that has already ended reports it at once.
	const stopEnd = session.onEnded(() => {
		stopped = true;
		stopPublication?.();
		stopAgents?.();
		client.report(ENDED_SUMMARY);
	});
	if (stopped) return stopEnd;
	stopPublication = projectWorkspaceRuntime.subscribe(schedule);
	stopAgents = agentObservationSource.subscribe(schedule);
	schedule();
	return stop;
}

function summarize(
	runningTerminals: number,
	observations: Iterable<Parameters<typeof resolveWorkbenchAgentDisplayState>[0]>,
): DesktopBackgroundStatusSummary {
	let attention = 0;
	let doneUnseen = 0;
	let failed = 0;
	let workingAgents = 0;
	for (const observation of observations) {
		switch (resolveWorkbenchAgentDisplayState(observation)) {
			case 'blocked':
				attention += 1;
				break;
			case 'done':
				doneUnseen += 1;
				break;
			case 'error':
				failed += 1;
				break;
			case 'working':
				workingAgents += 1;
				break;
			case 'idle':
			case 'unknown':
				break;
		}
	}
	return { attention, doneUnseen, failed, runningTerminals, workingAgents };
}
