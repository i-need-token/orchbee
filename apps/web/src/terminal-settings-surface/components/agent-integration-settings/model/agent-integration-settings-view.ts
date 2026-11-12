import { agentIntegrationMessages, type MessageDescriptor } from '@orchbee/i18n';

import {
	AGENT_INTEGRATION_NAME_MESSAGES,
	explainStartsAgent,
	type AgentIntegrationCommandPurpose,
	type AgentIntegrationDiagnosis,
	type AgentIntegrationEntryState,
	type AgentIntegrationExplanation,
	type AgentIntegrationId,
	type AgentIntegrationManagementFailureCode,
	type AgentIntegrationMarketplace,
	type AgentIntegrationSnapshotItem,
} from '../../../../agent-integration-management';

type AgentIntegrationStatusTone = 'danger' | 'neutral' | 'positive' | 'warning';

export interface AgentIntegrationSettingsRowView {
	readonly agentName: string;
	readonly checked: boolean;
	readonly description: string;
	/** The diagnosis shown under this row, or null while it is not open. */
	readonly diagnosis: AgentIntegrationDiagnosisView | null;
	readonly diagnosisTitle: string;
	readonly disabled: boolean;
	readonly explainDisabled: boolean;
	readonly explainLabel: string;
	readonly integrationId: AgentIntegrationId;
	readonly onCheckedChange: (checked: boolean) => void;
	readonly onExplain: () => void;
	readonly pending: boolean;
	readonly settingId: string;
	readonly status: string;
	readonly statusTone: AgentIntegrationStatusTone;
	readonly toggleLabel: string;
}

export interface AgentIntegrationSettingsView {
	readonly closeDiagnosisLabel: string;
	readonly disclosure: string;
	readonly explainText: string;
	readonly failure: string | null;
	readonly localSection: string;
	readonly onCloseDiagnosis: () => void;
	readonly onRefresh: () => void;
	readonly refreshDisabled: boolean;
	readonly refreshLabel: string;
	readonly remoteDescription: string;
	readonly remoteTitle: string;
	readonly rows: readonly AgentIntegrationSettingsRowView[];
}

export function resolveAgentIntegrationStatusView(
	integration: AgentIntegrationSnapshotItem,
	format: (descriptor: MessageDescriptor) => string,
): Pick<AgentIntegrationSettingsRowView, 'status' | 'statusTone'> {
	if (integration.managementMode === 'manual-only') {
		return {
			status: format(agentIntegrationMessages.statusManualOnly),
			statusTone: 'neutral',
		};
	}
	if (integration.issue === 'inspection-failed') {
		return {
			status: format(agentIntegrationMessages.statusInspectionFailed),
			statusTone: 'warning',
		};
	}
	if (integration.availability === 'not-found') {
		return {
			status: format(agentIntegrationMessages.statusNotFound),
			statusTone: 'neutral',
		};
	}
	if (integration.notChecked === true) {
		return {
			status: format(agentIntegrationMessages.statusNotChecked),
			statusTone: 'neutral',
		};
	}
	switch (integration.state) {
		case 'enabled':
			return {
				status: format(agentIntegrationMessages.statusEnabled),
				statusTone: 'positive',
			};
		case 'disabled':
			return {
				status: format(agentIntegrationMessages.statusDisabled),
				statusTone: 'neutral',
			};
		case 'not-installed':
			return {
				status: format(agentIntegrationMessages.statusNotInstalled),
				statusTone: 'neutral',
			};
		case 'update-required':
			return {
				status: format(agentIntegrationMessages.statusUpdateRequired),
				statusTone: 'warning',
			};
		case 'conflict':
			return {
				status: format(agentIntegrationMessages.statusConflict),
				statusTone: 'danger',
			};
		case 'unknown':
			return {
				status: format(agentIntegrationMessages.statusUnknown),
				statusTone: 'warning',
			};
	}
}

export function resolveAgentIntegrationFailureMessage(
	code: AgentIntegrationManagementFailureCode,
): MessageDescriptor {
	switch (code) {
		case 'agent-unavailable':
			return agentIntegrationMessages.failureAgentUnavailable;
		case 'busy':
			return agentIntegrationMessages.failureBusy;
		case 'conflict':
			return agentIntegrationMessages.failureConflict;
		case 'control-unavailable':
			return agentIntegrationMessages.failureControlUnavailable;
		case 'manual-action-required':
			return agentIntegrationMessages.failureManualActionRequired;
		case 'operation-failed':
			return agentIntegrationMessages.failureOperationFailed;
		case 'verification-failed':
			return agentIntegrationMessages.failureVerificationFailed;
	}
}

type Format = (descriptor: MessageDescriptor, values?: Readonly<Record<string, string>>) => string;
type FormatList = (items: readonly string[]) => string;

/**
 * What an explain report establishes. Manual setup is a definite fact of the platform; an
 * inspection that could not finish is an unknown result, never a missing integration.
 */
type AgentIntegrationFinding =
	| 'agent-missing'
	| 'conflict'
	| 'disabled'
	| 'manual-setup'
	| 'not-installed'
	| 'ready'
	| 'unknown-inspection-failed'
	| 'unknown-timed-out'
	| 'unknown-undetermined'
	| 'update-required';

interface AgentIntegrationDiagnosisFact {
	readonly label: string;
	readonly value: string;
}

export interface AgentIntegrationDiagnosisView {
	readonly detail: string | null;
	readonly facts: readonly AgentIntegrationDiagnosisFact[];
	readonly headline: string;
	readonly pending: boolean;
	/** What the check changed and started, or null while nothing has run. */
	readonly readOnly: string | null;
	readonly tone: AgentIntegrationStatusTone;
}

/** The question before a check that starts the Agent's program; nothing has run yet. */
export function resolveAgentIntegrationConsentView(
	agentName: string,
	format: Format,
): AgentIntegrationDiagnosisView {
	return {
		detail: format(agentIntegrationMessages.consentDetail, { agent: agentName }),
		facts: [],
		headline: format(agentIntegrationMessages.consentTitle, { agent: agentName }),
		pending: false,
		readOnly: null,
		tone: 'warning',
	};
}

function resolveAgentIntegrationFinding(
	explanation: AgentIntegrationExplanation,
): AgentIntegrationFinding {
	const { item } = explanation;
	if (item.managementMode === 'manual-only') return 'manual-setup';
	if (explanation.timedOut) return 'unknown-timed-out';
	if (item.issue === 'inspection-failed') return 'unknown-inspection-failed';
	if (item.availability === 'not-found') return 'agent-missing';
	switch (item.state) {
		case 'enabled':
			return 'ready';
		case 'disabled':
			return 'disabled';
		case 'not-installed':
			return 'not-installed';
		case 'update-required':
			return 'update-required';
		case 'conflict':
			return 'conflict';
		case 'unknown':
			return 'unknown-undetermined';
	}
}

export function resolveAgentIntegrationDiagnosisView(
	diagnosis: AgentIntegrationDiagnosis,
	format: Format,
	formatList: FormatList,
): AgentIntegrationDiagnosisView {
	const readOnly = format(agentIntegrationMessages.diagnosisReadOnly);
	switch (diagnosis.kind) {
		case 'pending':
			return {
				detail: null,
				facts: [],
				headline: format(agentIntegrationMessages.diagnosisPending),
				pending: true,
				readOnly,
				tone: 'neutral',
			};
		case 'failed':
			return {
				detail: format(resolveAgentIntegrationFailureMessage(diagnosis.code)),
				facts: [],
				headline: format(agentIntegrationMessages.findingUnknown),
				pending: false,
				readOnly,
				tone: 'warning',
			};
		case 'explained': {
			const { explanation } = diagnosis;
			const finding = resolveAgentIntegrationFinding(explanation);
			return {
				detail: format(FINDING_DETAILS[finding]),
				facts: explanationFacts(explanation, format, formatList),
				headline: format(FINDING_HEADLINES[finding]),
				pending: false,
				readOnly: explainStartsAgent(explanation.item)
					? format(agentIntegrationMessages.diagnosisStarted, {
							agent: format(AGENT_INTEGRATION_NAME_MESSAGES[diagnosis.integrationId]),
						})
					: readOnly,
				tone: FINDING_TONES[finding],
			};
		}
	}
}

const FINDING_HEADLINES: Readonly<Record<AgentIntegrationFinding, MessageDescriptor>> = {
	'agent-missing': agentIntegrationMessages.findingAgentMissing,
	conflict: agentIntegrationMessages.findingConflict,
	disabled: agentIntegrationMessages.findingDisabled,
	'manual-setup': agentIntegrationMessages.findingManualSetup,
	'not-installed': agentIntegrationMessages.findingNotInstalled,
	ready: agentIntegrationMessages.findingReady,
	'unknown-inspection-failed': agentIntegrationMessages.findingUnknown,
	'unknown-timed-out': agentIntegrationMessages.findingUnknown,
	'unknown-undetermined': agentIntegrationMessages.findingUnknown,
	'update-required': agentIntegrationMessages.findingUpdateRequired,
};

const FINDING_DETAILS: Readonly<Record<AgentIntegrationFinding, MessageDescriptor>> = {
	'agent-missing': agentIntegrationMessages.failureAgentUnavailable,
	conflict: agentIntegrationMessages.failureConflict,
	disabled: agentIntegrationMessages.findingDetailDisabled,
	'manual-setup': agentIntegrationMessages.findingDetailManualSetup,
	'not-installed': agentIntegrationMessages.findingDetailNotInstalled,
	ready: agentIntegrationMessages.findingDetailReady,
	'unknown-inspection-failed': agentIntegrationMessages.findingDetailInspectionFailed,
	'unknown-timed-out': agentIntegrationMessages.findingDetailTimedOut,
	'unknown-undetermined': agentIntegrationMessages.findingDetailUndetermined,
	'update-required': agentIntegrationMessages.findingDetailUpdateRequired,
};

const FINDING_TONES: Readonly<Record<AgentIntegrationFinding, AgentIntegrationStatusTone>> = {
	'agent-missing': 'warning',
	conflict: 'danger',
	disabled: 'neutral',
	'manual-setup': 'neutral',
	'not-installed': 'warning',
	ready: 'positive',
	'unknown-inspection-failed': 'warning',
	'unknown-timed-out': 'warning',
	'unknown-undetermined': 'warning',
	'update-required': 'warning',
};

const PLAN_STEPS = {
	'publish-adapter': agentIntegrationMessages.planPublishAdapter,
	'remove-entry': agentIntegrationMessages.planRemoveEntry,
	'write-entry': agentIntegrationMessages.planWriteEntry,
} as const satisfies Readonly<Record<string, MessageDescriptor>>;

/** The facts `orchbee agent integration explain` prints, in its order, without paths. */
function explanationFacts(
	explanation: AgentIntegrationExplanation,
	format: Format,
	formatList: FormatList,
): AgentIntegrationDiagnosisFact[] {
	const { detail, item } = explanation;
	const fact = (label: MessageDescriptor, value: MessageDescriptor | string) => ({
		label: format(label),
		value: typeof value === 'string' ? value : format(value),
	});
	const facts = [
		fact(agentIntegrationMessages.factAgent, PRESENCE[item.availability]),
		fact(
			agentIntegrationMessages.factIntegration,
			resolveAgentIntegrationStatusView(item, format).status,
		),
	];
	if (item.issue !== null) facts.push(fact(agentIntegrationMessages.factIssue, ISSUES[item.issue]));
	facts.push(fact(agentIntegrationMessages.factAdapter, ADAPTERS[explanation.adapter]));
	switch (detail.kind) {
		case 'cli':
		case 'codex':
			facts.push(
				fact(agentIntegrationMessages.factMarketplace, marketplace(detail.marketplace)),
				fact(agentIntegrationMessages.factPlugin, presenceFlag(detail.pluginPresent)),
			);
			break;
		case 'guarded':
			facts.push(
				fact(agentIntegrationMessages.factEntry, ENTRIES[detail.entry]),
				fact(
					agentIntegrationMessages.factAdapterSource,
					detail.sourceReady
						? agentIntegrationMessages.valueAdapterReady
						: agentIntegrationMessages.valueAdapterNotReady,
				),
			);
			break;
		case 'plain':
			break;
	}
	facts.push(
		fact(agentIntegrationMessages.factPlan, planText(explanation.plan, format, formatList)),
	);
	return facts;
}

const PLAN_PURPOSES: Readonly<Record<AgentIntegrationCommandPurpose, MessageDescriptor>> = {
	'add-plugin-source': agentIntegrationMessages.planPurposeAddPluginSource,
	'confirm-plugin': agentIntegrationMessages.planPurposeConfirmPlugin,
	'enable-plugin': agentIntegrationMessages.planPurposeEnablePlugin,
	'install-plugin': agentIntegrationMessages.planPurposeInstallPlugin,
	'remove-plugin': agentIntegrationMessages.planPurposeRemovePlugin,
	'remove-plugin-source': agentIntegrationMessages.planPurposeRemovePluginSource,
};

/**
 * The enable plan as one sentence fragment, as the TUI writes it: consecutive commands of one
 * program name it once and list what they do, and a repeated step shows once.
 */
function planText(
	plan: AgentIntegrationExplanation['plan'],
	format: Format,
	formatList: FormatList,
): string {
	if (plan.length === 0) return format(agentIntegrationMessages.planNone);
	const steps: string[] = [];
	for (let index = 0; index < plan.length;) {
		const step = plan[index];
		index += 1;
		if (step === undefined) break;
		let text: string;
		if (step.kind === 'command') {
			const purposes: string[] = [];
			const add = (purpose: AgentIntegrationCommandPurpose | null): void => {
				const purposeText = format(
					purpose === null ? agentIntegrationMessages.planPurposeOther : PLAN_PURPOSES[purpose],
				);
				if (!purposes.includes(purposeText)) purposes.push(purposeText);
			};
			add(step.purpose);
			for (let next = plan[index]; next?.kind === 'command' && next.command === step.command;) {
				add(next.purpose);
				index += 1;
				next = plan[index];
			}
			text = format(agentIntegrationMessages.planCommand, {
				command: step.command,
				purposes: formatList(purposes),
			});
		} else {
			text = format(PLAN_STEPS[step.kind]);
		}
		if (steps.at(-1) !== text) steps.push(text);
	}
	return steps.reduceRight((rest, first) =>
		format(agentIntegrationMessages.planJoin, { first, rest }),
	);
}

const PRESENCE: Readonly<Record<AgentIntegrationSnapshotItem['availability'], MessageDescriptor>> =
	{
		available: agentIntegrationMessages.valueFound,
		'not-found': agentIntegrationMessages.valueNotFound,
		unknown: agentIntegrationMessages.valueUnknown,
	};

const ISSUES: Readonly<
	Record<NonNullable<AgentIntegrationSnapshotItem['issue']>, MessageDescriptor>
> = {
	'inspection-failed': agentIntegrationMessages.statusInspectionFailed,
	'manual-action-required': agentIntegrationMessages.issueManualActionRequired,
	'unsupported-platform': agentIntegrationMessages.issueUnsupportedPlatform,
	'unsupported-version': agentIntegrationMessages.issueUnsupportedVersion,
};

const ADAPTERS: Readonly<Record<AgentIntegrationExplanation['adapter'], MessageDescriptor>> = {
	'binding-failed': agentIntegrationMessages.valueAdapterBindingFailed,
	'not-ready': agentIntegrationMessages.valueAdapterNotReady,
	ready: agentIntegrationMessages.valueAdapterReady,
	'source-absent': agentIntegrationMessages.valueAdapterSourceAbsent,
};

const ENTRIES: Readonly<Record<AgentIntegrationEntryState, MessageDescriptor>> = {
	conflict: agentIntegrationMessages.valueConflict,
	current: agentIntegrationMessages.valueCurrent,
	missing: agentIntegrationMessages.valueMissing,
	unknown: agentIntegrationMessages.valueUnknown,
	'update-required': agentIntegrationMessages.valueUpdateNeeded,
};

function marketplace(value: AgentIntegrationMarketplace | null): MessageDescriptor {
	switch (value) {
		case 'absent':
			return agentIntegrationMessages.valueNotAdded;
		case 'current':
			return agentIntegrationMessages.valueCurrent;
		case 'update-required':
			return agentIntegrationMessages.valueUpdateNeeded;
		case null:
			return agentIntegrationMessages.valueUnknown;
	}
}

function presenceFlag(present: boolean | null): MessageDescriptor {
	if (present === null) return agentIntegrationMessages.valueUnknown;
	return present
		? agentIntegrationMessages.valueInstalled
		: agentIntegrationMessages.valueNotInstalled;
}
