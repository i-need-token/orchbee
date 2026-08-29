import type { AgentSemanticEvent, AgentSessionSnapshot } from '@orchbee/agent';
import { agentObservationMessages, type IntlShape } from '@orchbee/i18n';
import type { TabModel } from '@orchbee/tab-strip';

import type { PanelWorkspaceStore } from '../components/workbench-panel-workspace/model/PanelWorkspaceStore';
import { collectPanelLeaves } from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type { WorkbenchAgentEventObservation } from '../types/workbench-agent-notification';
import { formatWorkbenchAgentProvider } from './formatWorkbenchAgentProvider';

interface ProjectWorkbenchAgentNotificationOptions {
	readonly event: AgentSemanticEvent;
	readonly intl: IntlShape;
	readonly panelWorkspaceStore: PanelWorkspaceStore;
	readonly session: AgentSessionSnapshot;
	readonly tabModel: TabModel;
}

export function projectWorkbenchAgentNotification({
	event,
	intl,
	panelWorkspaceStore,
	session,
	tabModel,
}: ProjectWorkbenchAgentNotificationOptions): WorkbenchAgentEventObservation | null {
	if (
		event.type !== 'attention-requested' &&
		(event.type !== 'turn-outcome' || (event.outcome !== 'failed' && event.outcome !== 'succeeded'))
	) {
		return null;
	}
	const state = panelWorkspaceStore.getSnapshot();
	const activeTabId = panelWorkspaceStore.getActiveTabId();
	const attachments = new Map(
		session.presentationAttachments
			.filter((attachment) => attachment.navigable)
			.map((attachment) => [attachment.presentationId, attachment]),
	);
	const destinations = state.tabs
		.flatMap((tab) =>
			collectPanelLeaves(tab.root).flatMap((panel) => {
				const localPanelVisible =
					tab.tabId === activeTabId &&
					(tab.maximizedPanelId === null || tab.maximizedPanelId === panel.id);
				const displayedPresentationId =
					panel.activeChildPresentationId ?? panel.primaryPresentation.presentationId;
				const presentations = [
					{ child: false, presentationId: panel.primaryPresentation.presentationId },
					...panel.childPresentations.map((presentation) => ({
						child: true,
						presentationId: presentation.presentationId,
					})),
				];
				return presentations.flatMap(({ child, presentationId }) => {
					const attachment = attachments.get(presentationId);
					return attachment === undefined
						? []
						: [
								{
									attachment,
									child,
									panel,
									presentationId,
									tabId: tab.tabId,
									visible:
										localPanelVisible &&
										displayedPresentationId === presentationId &&
										attachment.visibility !== 'hidden',
								},
							];
				});
			}),
		)
		.sort((left, right) =>
			`${left.tabId}\0${left.panel.id}\0${left.attachment.presentationId}`.localeCompare(
				`${right.tabId}\0${right.panel.id}\0${right.attachment.presentationId}`,
			),
		);
	const destination = destinations[0];
	if (destination === undefined) return null;

	const provider = formatWorkbenchAgentProvider(intl, [session.providerId]);
	let body: string;
	let title: string;
	if (event.type === 'attention-requested') {
		body = intl.formatMessage(agentObservationMessages.notificationAttentionBody, { provider });
		title = intl.formatMessage(agentObservationMessages.notificationAttentionTitle);
	} else if (event.outcome === 'failed') {
		body = intl.formatMessage(agentObservationMessages.notificationFailedBody, { provider });
		title = intl.formatMessage(agentObservationMessages.notificationFailedTitle);
	} else {
		body = intl.formatMessage(agentObservationMessages.notificationSucceededBody, { provider });
		title = intl.formatMessage(agentObservationMessages.notificationSucceededTitle);
	}

	return {
		event,
		notification: {
			body,
			onActivate: () => {
				tabModel.selectTab(destination.tabId);
				if (destination.child) {
					panelWorkspaceStore.transition({
						childPresentationId: destination.presentationId,
						panelId: destination.panel.id,
						tabId: destination.tabId,
						type: 'child-terminal-activated',
					});
					return;
				}
				panelWorkspaceStore.transition({
					panelId: destination.panel.id,
					tabId: destination.tabId,
					type: 'primary-terminal-shown',
				});
				panelWorkspaceStore.transition({
					panelId: destination.panel.id,
					tabId: destination.tabId,
					type: 'panel-focused',
				});
			},
			tag: `orchbee-agent:${event.eventId}`,
			title,
		},
		visible: destinations.some((candidate) => candidate.visible),
	};
}
