import type {
	DesktopSshEnvironmentConnectionState,
	DesktopSshEnvironmentDiscoveryIssue,
	DesktopSshEnvironmentId,
} from '@orchbee/desktop-client-protocol';
import {
	sshConnectionManagementMessages,
	useOrchbeeIntl,
	type MessageDescriptor,
} from '@orchbee/i18n';
import * as React from 'react';

import type {
	DesktopSshConnectionManagementControl,
	DesktopSshConnectionManagementFailure,
} from '../../../../ssh-connection-management';

type ConnectionStatusTone = 'danger' | 'muted' | 'positive' | 'warning';

interface SshConnectionSettingsManagedRowView {
	readonly actionDisabled: boolean;
	readonly actionLabel: string;
	readonly destination: string;
	readonly environmentId: DesktopSshEnvironmentId;
	readonly name: string;
	readonly onAction: () => void;
	readonly projectUsage: string;
	readonly source: string;
	readonly status: string;
	readonly statusTone: ConnectionStatusTone;
}

interface SshConnectionSettingsSuggestionView {
	readonly actionDisabled: boolean;
	readonly actionLabel: string;
	readonly destination: string;
	readonly onAdd: () => void;
	readonly source: string;
}

interface SshConnectionSettingsView {
	readonly addActionDisabled: boolean;
	readonly addActionLabel: string;
	readonly destination: string;
	readonly discoveryWarning: string | null;
	readonly failure: string | null;
	readonly managed: readonly SshConnectionSettingsManagedRowView[];
	readonly onDestinationChange: (event: React.ChangeEvent<HTMLInputElement>) => void;
	readonly onDismissFailure: () => void;
	readonly onRefresh: () => void;
	readonly onSubmit: (event: React.FormEvent<HTMLFormElement>) => void;
	readonly registryWarning: string | null;
	readonly refreshDisabled: boolean;
	readonly suggestions: readonly SshConnectionSettingsSuggestionView[];
}

const CONNECTION_STATUS_MESSAGES: Readonly<
	Record<DesktopSshEnvironmentConnectionState, MessageDescriptor>
> = {
	blocked: sshConnectionManagementMessages.statusBlocked,
	connected: sshConnectionManagementMessages.statusConnected,
	connecting: sshConnectionManagementMessages.statusConnecting,
	disconnected: sshConnectionManagementMessages.statusDisconnected,
	'interaction-required': sshConnectionManagementMessages.statusInteractionRequired,
	retrying: sshConnectionManagementMessages.statusRetrying,
	unavailable: sshConnectionManagementMessages.statusUnavailable,
};

const CONNECTION_STATUS_TONES: Readonly<
	Record<DesktopSshEnvironmentConnectionState, ConnectionStatusTone>
> = {
	blocked: 'warning',
	connected: 'positive',
	connecting: 'muted',
	disconnected: 'muted',
	'interaction-required': 'warning',
	retrying: 'muted',
	unavailable: 'danger',
};

const DISCOVERY_ISSUE_MESSAGES: Readonly<
	Record<DesktopSshEnvironmentDiscoveryIssue, MessageDescriptor>
> = {
	'invalid-config': sshConnectionManagementMessages.discoveryInvalidConfig,
	'read-failed': sshConnectionManagementMessages.discoveryReadFailed,
	truncated: sshConnectionManagementMessages.discoveryTruncated,
};

const FAILURE_MESSAGES: Readonly<Record<DesktopSshConnectionManagementFailure, MessageDescriptor>> =
	{
		busy: sshConnectionManagementMessages.failureBusy,
		connection: sshConnectionManagementMessages.failureConnection,
		control: sshConnectionManagementMessages.failureControl,
		storage: sshConnectionManagementMessages.failureStorage,
		target: sshConnectionManagementMessages.failureTarget,
	};

export function useSshConnectionSettings(
	control: DesktopSshConnectionManagementControl,
): SshConnectionSettingsView {
	const intl = useOrchbeeIntl();
	const snapshot = React.useSyncExternalStore(
		control.subscribeManagement,
		control.getManagementSnapshot,
		control.getManagementSnapshot,
	);
	const [destination, setDestination] = React.useState('');

	React.useEffect(() => {
		control.refreshConnections();
	}, [control]);

	const registryUnavailable = snapshot.catalog.registryIssue !== null;
	const operationPending = snapshot.pending !== null;
	const addDestination = (candidate: string, clearInput: boolean): void => {
		if (registryUnavailable || operationPending) return;
		const normalized = candidate.trim();
		if (normalized.length === 0) return;
		void control.addConnection(normalized).then((added) => {
			if (!added || !clearInput) return;
			setDestination((current) => (current.trim() === normalized ? '' : current));
		});
	};

	return {
		addActionDisabled: registryUnavailable || operationPending || destination.trim().length === 0,
		addActionLabel:
			snapshot.pending?.kind === 'add'
				? intl.formatMessage(sshConnectionManagementMessages.actionAdding)
				: intl.formatMessage(sshConnectionManagementMessages.actionAdd),
		destination,
		discoveryWarning:
			snapshot.catalog.discoveryIssue === null
				? null
				: intl.formatMessage(DISCOVERY_ISSUE_MESSAGES[snapshot.catalog.discoveryIssue]),
		failure:
			snapshot.failure === null ? null : intl.formatMessage(FAILURE_MESSAGES[snapshot.failure]),
		managed: snapshot.catalog.managed.map((connection) => {
			const disconnecting =
				snapshot.pending?.kind === 'disconnect' &&
				snapshot.pending.environmentId === connection.environmentId;
			const connecting =
				connection.state === 'connecting' ||
				connection.state === 'retrying' ||
				connection.state === 'interaction-required';
			const connected = connection.state === 'connected';
			const actionable =
				connected || connection.state === 'blocked' || connection.state === 'disconnected';
			let actionMessage: MessageDescriptor = sshConnectionManagementMessages.actionConnect;
			if (connected) actionMessage = sshConnectionManagementMessages.actionDisconnect;
			else if (connection.state === 'blocked') {
				actionMessage = sshConnectionManagementMessages.actionRetry;
			}
			return {
				actionDisabled: registryUnavailable || operationPending || connecting || !actionable,
				actionLabel: disconnecting
					? intl.formatMessage(sshConnectionManagementMessages.actionDisconnecting)
					: intl.formatMessage(actionMessage),
				destination: connection.destination,
				environmentId: connection.environmentId,
				name: connection.name,
				onAction: (): void => {
					if (registryUnavailable || operationPending || connecting || !actionable) return;
					if (connected) control.disconnectEnvironment(connection.environmentId);
					else control.connectEnvironment(connection.environmentId, connection.name);
				},
				projectUsage: intl.formatMessage(sshConnectionManagementMessages.managedProjectCount, {
					count: connection.projectCount,
				}),
				source: intl.formatMessage(sshConnectionManagementMessages.sourceOrchbee),
				status: intl.formatMessage(CONNECTION_STATUS_MESSAGES[connection.state]),
				statusTone: CONNECTION_STATUS_TONES[connection.state],
			};
		}),
		onDestinationChange: (event): void => {
			setDestination(event.currentTarget.value);
			if (snapshot.failure !== null) control.clearFailure();
		},
		onDismissFailure: (): void => control.clearFailure(),
		onRefresh: (): void => {
			if (!operationPending) control.refreshConnections();
		},
		onSubmit: (event): void => {
			event.preventDefault();
			addDestination(destination, true);
		},
		registryWarning: registryUnavailable
			? intl.formatMessage(sshConnectionManagementMessages.registryUnavailable)
			: null,
		refreshDisabled: operationPending,
		suggestions: snapshot.catalog.suggestions.map((suggestion) => {
			const adding =
				snapshot.pending?.kind === 'add' && snapshot.pending.destination === suggestion.destination;
			return {
				actionDisabled: registryUnavailable || operationPending,
				actionLabel: intl.formatMessage(
					adding
						? sshConnectionManagementMessages.actionAdding
						: sshConnectionManagementMessages.actionAddHost,
				),
				destination: suggestion.destination,
				onAdd: (): void => addDestination(suggestion.destination, false),
				source: intl.formatMessage(sshConnectionManagementMessages.sourceSystem),
			};
		}),
	};
}
