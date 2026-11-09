import type {
	ActionAvailabilityContext,
	ActionDiscoveryResult,
	ActionExecutionContext,
	ActionInvocation,
	ActionMessageDescriptor,
	ActionRuntime,
	ActionScope,
	PreparedActionInvocation,
} from '@orchbee/actions';
import { workbenchMessages } from '@orchbee/i18n';

import { WORKBENCH_DESCENDANT_ACTION_DISCOVERY_INPUTS } from './workbench-descendant-action-discovery';

export type DescendantActionPreparation =
	| { readonly invocation: PreparedActionInvocation; readonly kind: 'ready' }
	| { readonly kind: 'unavailable'; readonly reason: ActionMessageDescriptor };

type ReadyDiscovery = Extract<ActionDiscoveryResult, { kind: 'ready' }>;

/**
 * Prepares the lower Action a composite runs with this input. A disabled lower Action lends its own
 * reason, so the composite says why as its owner would; only a lower Action that no surface has
 * registered leaves the host reason.
 */
export function prepareDescendantAction({
	actionId,
	context,
	input,
	runtime,
	scope,
}: {
	readonly actionId: string;
	readonly context: ActionAvailabilityContext | ActionExecutionContext;
	readonly input: unknown;
	readonly runtime: ActionRuntime;
	readonly scope: ActionScope;
}): DescendantActionPreparation {
	const candidates = runtime
		.discover(discoveryInvocation(context, scope), {
			includeDescendants: true,
			inputByActionId: {
				...WORKBENCH_DESCENDANT_ACTION_DISCOVERY_INPUTS,
				[actionId]: input,
			},
		})
		.filter(
			(result): result is ReadyDiscovery =>
				result.kind === 'ready' && result.descriptor.id === actionId,
		);
	const enabled = candidates.find((result) => result.availability.kind === 'enabled');
	if (enabled !== undefined) return { invocation: enabled.invocation, kind: 'ready' };
	for (const { availability } of candidates) {
		if (availability.kind === 'disabled') {
			return { kind: 'unavailable', reason: availability.reason };
		}
	}
	return { kind: 'unavailable', reason: workbenchMessages.actionUnavailable };
}

function discoveryInvocation(
	context: ActionAvailabilityContext | ActionExecutionContext,
	scope: ActionScope,
): ActionInvocation {
	return {
		...(context.capabilities === undefined ? {} : { capabilities: context.capabilities }),
		...(context.focus === undefined ? {} : { focus: context.focus }),
		scope,
		source: context.source,
		...(context.target === undefined ? {} : { target: context.target }),
	};
}
