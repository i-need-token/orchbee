import { useMemoizedFn } from '@orchbee/foundation/react';
import {
	agentObservationMessages,
	type IntlShape,
	useOrchbeeIntl,
	workbenchMessages,
} from '@orchbee/i18n';
import type { TabStatusAdornment, TabStatusAdornmentKind } from '@orchbee/tab-strip';
import * as React from 'react';

import { AgentProviderIcon } from '../../agent-provider-icon';
import { WorkbenchAgentStateGlyph } from '../components/workbench-agent-state-glyph/WorkbenchAgentStateGlyph';
import {
	collectPanelLeaves,
	getDisplayedTerminal,
} from '../components/workbench-panel-workspace/model/panel-workspace-state';
import type {
	WorkbenchProjectWorkspaceRuntime,
	WorkbenchTerminalContext,
} from '../model/WorkbenchProjectWorkspaceRuntime';
import {
	mostUrgentWorkbenchAgentState,
	projectWorkbenchAgentOverview,
	type WorkbenchAgentNavigationInput,
} from '../model/project-workbench-agent-overview';
import { resolveWorkbenchAgentTitle } from '../model/resolveWorkbenchAgentTitle';
import { WORKBENCH_AGENT_CONFIRMATION_WAITS } from '../model/workbench-agent-confirmation-news';
import type {
	WorkbenchAgentConfirmation,
	WorkbenchAgentConfirmations,
} from '../types/workbench-agent-confirmations';
import type { WorkbenchAgentNotificationObserver } from '../types/workbench-agent-notification';
import type {
	WorkbenchAgentNames,
	WorkbenchAgentNameSource,
	WorkbenchAgentState,
	WorkbenchAgentSource,
} from '../types/workbench-agent-observation';
import type {
	WorkbenchAgentConfirmationOfferView,
	WorkbenchWorkspaceAgentOverviewView,
} from '../types/workbench-agent-overview';
import { formatWorkbenchAgentLocation } from '../utils/formatWorkbenchAgentLocation';
import {
	formatWorkbenchAgentGroupName,
	formatWorkbenchAgentName,
	listWorkbenchAgentProviderNames,
	workbenchAgentTitleNamesAgent,
} from '../utils/workbench-agent-names';
import {
	formatWorkbenchAgentStatus,
	type WorkbenchAgentStatusView,
} from '../utils/formatWorkbenchAgentStatus';
import { subscribeToNothing } from '../utils/subscribeToNothing';
import { useWorkbenchAgentAttention } from './useWorkbenchAgentAttention';
import { useWorkbenchAgentNotifications } from './useWorkbenchAgentNotifications';

interface UseWorkbenchAgentPresentationOptions {
	/** How the Server answers Agent confirmations; absent where it answers none. */
	readonly confirmations?: WorkbenchAgentConfirmations | undefined;
	/** The names the Server reports for Agents, which an Agent row's title comes from first. */
	readonly nameSource?: WorkbenchAgentNameSource | undefined;
	readonly navigateAgent: (input: WorkbenchAgentNavigationInput) => void;
	readonly notificationObserver: WorkbenchAgentNotificationObserver | undefined;
	readonly projectWorkspaceRuntime: WorkbenchProjectWorkspaceRuntime;
	readonly root: React.RefObject<HTMLElement | null>;
	readonly source: WorkbenchAgentSource | undefined;
	/** The active Workspace's context, whose Tabs carry Agent marks; null while none is open. */
	readonly terminalContext: WorkbenchTerminalContext | null;
}

interface WorkbenchAgentPresentationView {
	/**
	 * The offer the status notice shows: the focused Panel's Agent first, then another Agent the
	 * active Tab shows, as the TUI picks its key's subject.
	 */
	readonly confirmationOffer: WorkbenchAgentConfirmationOfferView | null;
	/** Each reported Agent's status view by Presentation ID; a Panel header draws its displayed terminal's. */
	readonly presentationAgentStatuses: ReadonlyMap<string, WorkbenchAgentStatusView>;
	readonly tabIdentityIcons: ReadonlyMap<string, React.ReactNode>;
	readonly tabStatusAdornments: ReadonlyMap<string, Readonly<TabStatusAdornment>>;
	/** Folds or unfolds one Workspace's Agent rows. */
	readonly toggleWorkspaceAgents: (workspaceId: string) => void;
	readonly workspaceAgentOverviews: readonly WorkbenchWorkspaceAgentOverviewView[];
}

const NO_PANEL_TABS: readonly [] = [];
const NO_AGENT_NAMES: ReadonlyMap<string, WorkbenchAgentNames> = new Map();
const NO_CONFIRMATIONS: ReadonlyMap<string, WorkbenchAgentConfirmation> = new Map();
const NO_WORKSPACE_AGENT_OVERVIEWS: readonly WorkbenchWorkspaceAgentOverviewView[] = [];

/** The active Tab's focused Panel's terminal: the open Workspace's current Agent, if it is one. */
function focusedPresentationId(
	panelTabs: ReturnType<WorkbenchTerminalContext['panelWorkspaceStore']['getSnapshot']>['tabs'],
	activeTabId: string | null,
): string | null {
	const active = panelTabs.find((tab) => tab.tabId === activeTabId);
	const panel =
		active === undefined
			? undefined
			: collectPanelLeaves(active.root).find((leaf) => leaf.id === active.focusedPanelId);
	return panel === undefined
		? null
		: (panel.activeChildPresentationId ?? panel.primaryPresentation.presentationId);
}

const TAB_ADORNMENT_KINDS: Readonly<Record<WorkbenchAgentState, TabStatusAdornmentKind>> = {
	blocked: 'attention',
	done: 'success',
	error: 'error',
	idle: 'idle',
	opening: 'active',
	unknown: 'idle',
	working: 'active',
};

/** One Tab's Agent mark and the facts it is drawn from. */
interface TabAgentPresentation {
	readonly agents: readonly string[];
	readonly gradientPrefix: string;
	readonly hasUnread: boolean;
	readonly identityIcon: React.ReactNode;
	readonly state: WorkbenchAgentState;
	readonly statusAdornment: Readonly<TabStatusAdornment>;
}

interface TabAgentPresentations {
	readonly byTab: ReadonlyMap<string, TabAgentPresentation>;
	readonly identityIcons: ReadonlyMap<string, React.ReactNode>;
	/** The messages the marks are written in; another locale draws every mark again. */
	readonly intl: IntlShape | null;
	readonly statusAdornments: ReadonlyMap<string, Readonly<TabStatusAdornment>>;
}

const NO_TAB_AGENT_PRESENTATIONS: TabAgentPresentations = {
	byTab: new Map(),
	identityIcons: new Map(),
	intl: null,
	statusAdornments: new Map(),
};

function sameStrings(left: readonly string[], right: readonly string[]): boolean {
	return left.length === right.length && left.every((value, index) => value === right[index]);
}

/** Whether both list the same Tabs, in the same order, with the same mark objects. */
function sameTabAgentPresentations(
	left: ReadonlyMap<string, TabAgentPresentation>,
	right: ReadonlyMap<string, TabAgentPresentation>,
): boolean {
	const rightEntries = [...right];
	return (
		left.size === right.size &&
		[...left].every(
			([tabId, presentation], index) =>
				rightEntries[index]?.[0] === tabId && rightEntries[index]?.[1] === presentation,
		)
	);
}

export function useWorkbenchAgentPresentation({
	confirmations,
	nameSource,
	navigateAgent,
	notificationObserver,
	projectWorkspaceRuntime,
	root,
	source,
	terminalContext,
}: UseWorkbenchAgentPresentationOptions): WorkbenchAgentPresentationView {
	const agentIconGradientPrefix = React.useId().replaceAll(':', '');
	const intl = useOrchbeeIntl();
	const painted = useWorkbenchAgentNotifications({
		intl,
		navigateAgent,
		observer: notificationObserver,
		projectWorkspaceRuntime,
		source,
	});
	const { presentation: snapshot, unread } = useWorkbenchAgentAttention(
		source,
		terminalContext,
		root,
		painted,
	);
	const readAgentNames = () => nameSource?.getSnapshot() ?? NO_AGENT_NAMES;
	const agentNames = React.useSyncExternalStore(
		nameSource?.subscribe ?? subscribeToNothing,
		readAgentNames,
		readAgentNames,
	);
	const readConfirmations = (): ReadonlyMap<string, WorkbenchAgentConfirmation> =>
		confirmations?.getSnapshot() ?? NO_CONFIRMATIONS;
	const confirmationRows = React.useSyncExternalStore(
		confirmations?.subscribe ?? subscribeToNothing,
		readConfirmations,
		readConfirmations,
	);
	const panelWorkspaceStore = terminalContext?.panelWorkspaceStore;
	const readPanelTabs = () => panelWorkspaceStore?.getSnapshot().tabs ?? NO_PANEL_TABS;
	const panelTabs = React.useSyncExternalStore(
		panelWorkspaceStore?.subscribe ?? subscribeToNothing,
		readPanelTabs,
		readPanelTabs,
	);
	const tabModel = terminalContext?.tabModel;
	const readActiveTabId = () => tabModel?.getSnapshot().activeTabId ?? null;
	const activeTabId = React.useSyncExternalStore(
		tabModel?.subscribe ?? subscribeToNothing,
		readActiveTabId,
		readActiveTabId,
	);
	const currentPresentationId = focusedPresentationId(panelTabs, activeTabId);
	const confirmationOffer = React.useMemo((): WorkbenchAgentConfirmationOfferView | null => {
		const active = panelTabs.find((tab) => tab.tabId === activeTabId);
		if (active === undefined) return null;
		const shown = collectPanelLeaves(active.root).map(
			(panel) => getDisplayedTerminal(panel).presentationId,
		);
		const focused = shown.filter((presentationId) => presentationId === currentPresentationId);
		for (const presentationId of [...focused, ...shown]) {
			const confirmation = confirmationRows.get(presentationId);
			if (confirmation?.offer === true) {
				return {
					agentLabel: formatWorkbenchAgentName(intl, confirmation.agent),
					presentationId,
				};
			}
		}
		return null;
	}, [activeTabId, confirmationRows, currentPresentationId, intl, panelTabs]);
	const terminalLayoutRevision = React.useSyncExternalStore(
		projectWorkspaceRuntime.subscribeTerminalTopology,
		projectWorkspaceRuntime.getTerminalLayoutRevision,
		projectWorkspaceRuntime.getTerminalLayoutRevision,
	);
	// As in the TUI, every Workspace shows its Agents until this Client folds them, and a fold lasts
	// while its Workspace does, through a hidden sidebar and Agents that come and go.
	const [foldedWorkspaceIds, setFoldedWorkspaceIds] = React.useState<ReadonlySet<string>>(
		() => new Set(),
	);
	const toggleWorkspaceAgents = useMemoizedFn((workspaceId: string): void => {
		const present = new Set(projectWorkspaceRuntime.getSnapshot().workspaces.map(({ id }) => id));
		setFoldedWorkspaceIds((current) => {
			const next = new Set([...current].filter((id) => present.has(id)));
			if (!next.delete(workspaceId)) next.add(workspaceId);
			return next;
		});
	});
	// The Tab strip skips a Tab whose mark objects keep their identity, so a Tab whose Agent facts
	// did not change keeps the objects of the last result. A result from a render React discards
	// is still right for its facts, so it may be reused as well.
	const lastTabPresentation = React.useRef(NO_TAB_AGENT_PRESENTATIONS);
	const tabPresentation = React.useMemo(() => {
		const previous = lastTabPresentation.current;
		const reusable = previous.intl === intl ? previous.byTab : NO_TAB_AGENT_PRESENTATIONS.byTab;
		const byTab = new Map<string, TabAgentPresentation>();
		const unreadLabel = intl.formatMessage(agentObservationMessages.statusUnread);
		for (const tab of panelTabs) {
			const presentationIds = collectPanelLeaves(tab.root).flatMap((panel) => [
				panel.primaryPresentation.presentationId,
				...panel.childPresentations.map((presentation) => presentation.presentationId),
			]);
			const observations = presentationIds.flatMap((id) => {
				const observation = snapshot.presentations.get(id);
				return observation === undefined ? [] : [observation];
			});
			const state = mostUrgentWorkbenchAgentState(
				observations.map((observation) => observation.state),
			);
			if (state === null) continue;
			const agents = [...new Set(observations.map((observation) => observation.agent))];
			const gradientPrefix = `${agentIconGradientPrefix}-${byTab.size}`;
			const hasUnread = presentationIds.some((id) => unread.has(id));
			const prior = reusable.get(tab.tabId);
			if (
				prior !== undefined &&
				prior.state === state &&
				prior.hasUnread === hasUnread &&
				prior.gradientPrefix === gradientPrefix &&
				sameStrings(prior.agents, agents)
			) {
				byTab.set(tab.tabId, prior);
				continue;
			}
			const identityIcon = React.createElement(AgentProviderIcon, { agents, gradientPrefix });
			const status = formatWorkbenchAgentStatus(intl, state);
			byTab.set(tab.tabId, {
				agents,
				gradientPrefix,
				hasUnread,
				identityIcon,
				state,
				statusAdornment: {
					accessibleLabel: `${formatWorkbenchAgentGroupName(intl, agents)}: ${status.status}${hasUnread ? ` · ${unreadLabel}` : ''}`,
					mark: React.createElement(WorkbenchAgentStateGlyph, { status }),
					icon: identityIcon,
					kind: TAB_ADORNMENT_KINDS[state],
					unreadLabel: hasUnread ? unreadLabel : undefined,
				},
			});
		}
		const next: TabAgentPresentations =
			previous.intl === intl && sameTabAgentPresentations(previous.byTab, byTab)
				? previous
				: {
						byTab,
						identityIcons: new Map(
							[...byTab].map(([tabId, presentation]) => [tabId, presentation.identityIcon]),
						),
						intl,
						statusAdornments: new Map(
							[...byTab].map(([tabId, presentation]) => [tabId, presentation.statusAdornment]),
						),
					};
		lastTabPresentation.current = next;
		return next;
	}, [agentIconGradientPrefix, intl, panelTabs, snapshot, unread]);
	const presentationAgentStatuses = React.useMemo(() => {
		const statuses = new Map<string, WorkbenchAgentStatusView>();
		for (const [presentationId, observation] of snapshot.presentations) {
			statuses.set(presentationId, formatWorkbenchAgentStatus(intl, observation.state));
		}
		return statuses;
	}, [intl, snapshot]);
	const workspaceAgentOverviews = React.useMemo(() => {
		if (terminalLayoutRevision !== projectWorkspaceRuntime.getTerminalLayoutRevision()) {
			return NO_WORKSPACE_AGENT_OVERVIEWS;
		}
		const overview = projectWorkbenchAgentOverview(snapshot, projectWorkspaceRuntime);
		// No Agents keeps one empty list, so the sidebar sees no change from a Tab switch.
		if (overview.length === 0) return NO_WORKSPACE_AGENT_OVERVIEWS;
		const unreadLabel = intl.formatMessage(agentObservationMessages.statusUnread);
		return overview.map((workspace): WorkbenchWorkspaceAgentOverviewView => {
			const count = workspace.rows.length;
			const states = workspace.rowsByUrgency.map(({ observation }) => observation.state);
			// A row's name as its expanded row shows it: its title, then its provider.
			const rowNames = ({ destination, observation }: (typeof workspace.rows)[number]) => {
				const agentLabel = formatWorkbenchAgentName(intl, observation.agent);
				const names = agentNames.get(destination.presentationId);
				const titleLabel =
					resolveWorkbenchAgentTitle(
						destination.terminalNames,
						// What the Server read for another incarnation names nothing in this one.
						names?.binding === observation.binding ? names : undefined,
						listWorkbenchAgentProviderNames(intl, observation.agent),
						workbenchAgentTitleNamesAgent(observation.agent),
					) ?? '';
				return { agentLabel, titleLabel };
			};
			// What a control that opens the Agent's terminal is named, its row or its folded glyph.
			const navigationLabel = ({ destination, observation }: (typeof workspace.rows)[number]) =>
				intl.formatMessage(workbenchMessages.workspaceAgentNavigationLabel, {
					location: formatWorkbenchAgentLocation(intl, destination),
					provider: formatWorkbenchAgentName(intl, observation.agent),
					status: formatWorkbenchAgentStatus(intl, observation.state).status,
				});
			return {
				agentLabel: formatWorkbenchAgentGroupName(intl, workspace.agents),
				collapseLabel: intl.formatMessage(workbenchMessages.workspaceAgentCollapse, {
					count,
					workspace: workspace.workspaceName,
				}),
				countLabel: intl.formatMessage(workbenchMessages.workspaceAgentCount, { count }),
				expandLabel: intl.formatMessage(workbenchMessages.workspaceAgentExpand, {
					count,
					workspace: workspace.workspaceName,
				}),
				expanded: !foldedWorkspaceIds.has(workspace.workspaceId),
				foldedStatus: {
					glyphs: workspace.rowsByUrgency.map((row) => {
						const { agentLabel, titleLabel } = rowNames(row);
						const status = formatWorkbenchAgentStatus(intl, row.observation.state);
						return {
							accessibleLabel: navigationLabel(row),
							identity: titleLabel === '' ? agentLabel : `${titleLabel} · ${agentLabel}`,
							presentationId: row.destination.presentationId,
							shape: status.shape,
							status: status.status,
							tone: status.tone,
						};
					}),
					label: intl.formatList(
						[...new Set(states)].map((state) => formatWorkbenchAgentStatus(intl, state).status),
						{ style: 'narrow', type: 'conjunction' },
					),
				},
				projectId: workspace.projectId,
				rows: workspace.rows.map((row) => {
					const { destination, observation } = row;
					const { agentLabel, titleLabel } = rowNames(row);
					const status = formatWorkbenchAgentStatus(intl, observation.state);
					const confirmation = confirmationRows.get(destination.presentationId);
					return {
						confirmation:
							confirmation === undefined
								? undefined
								: {
										agentTypeLabel: intl.formatMessage(
											workbenchMessages.agentConfirmationAgentType,
											{ agent: formatWorkbenchAgentName(intl, confirmation.agent) },
										),
										choice: confirmation.choice,
									},
						statusReason:
							confirmation === undefined || confirmation.wait === null
								? undefined
								: intl.formatMessage(workbenchMessages.agentConfirmationWaitStatus, {
										reason: intl.formatMessage(
											WORKBENCH_AGENT_CONFIRMATION_WAITS[confirmation.wait],
										),
										status: status.status,
									}),
						accessibleLabel: navigationLabel(row),
						agentLabel,
						current: destination.presentationId === currentPresentationId,
						finished: observation.state === 'done',
						panelId: destination.panelId,
						presentationId: destination.presentationId,
						primary: destination.terminalPosition.kind === 'primary',
						providerId: observation.agent,
						status,
						tabId: destination.tabId,
						terminalContextId: destination.terminalContextId,
						titleLabel,
						unreadLabel: unread.has(destination.presentationId) ? unreadLabel : undefined,
					};
				}),
				status: formatWorkbenchAgentStatus(intl, workspace.state),
				unreadLabel: workspace.rows.some((row) => unread.has(row.destination.presentationId))
					? unreadLabel
					: undefined,
				workspaceId: workspace.workspaceId,
			};
		});
	}, [
		agentNames,
		confirmationRows,
		currentPresentationId,
		foldedWorkspaceIds,
		intl,
		projectWorkspaceRuntime,
		snapshot,
		terminalLayoutRevision,
		unread,
	]);
	return React.useMemo(
		() => ({
			confirmationOffer,
			presentationAgentStatuses,
			tabIdentityIcons: tabPresentation.identityIcons,
			tabStatusAdornments: tabPresentation.statusAdornments,
			toggleWorkspaceAgents,
			workspaceAgentOverviews,
		}),
		[
			confirmationOffer,
			presentationAgentStatuses,
			tabPresentation,
			workspaceAgentOverviews,
			toggleWorkspaceAgents,
		],
	);
}
