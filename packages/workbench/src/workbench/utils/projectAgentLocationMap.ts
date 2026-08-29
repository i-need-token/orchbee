import {
	projectAgentLocation,
	type AgentLocationPresentation,
	type AgentRuntimeSnapshot,
} from '@orchbee/agent';

export function projectAgentLocationMap(
	snapshot: AgentRuntimeSnapshot,
): ReadonlyMap<string, AgentLocationPresentation> {
	const locations = new Map<string, AgentLocationPresentation>();
	for (const session of snapshot.sessions) {
		for (const attachment of session.presentationAttachments) {
			const presentationId = attachment.presentationId;
			if (locations.has(presentationId)) continue;
			const location = projectAgentLocation(snapshot, presentationId);
			if (location !== null && location.status.kind !== 'ended') {
				locations.set(presentationId, location);
			}
		}
	}
	return locations;
}
