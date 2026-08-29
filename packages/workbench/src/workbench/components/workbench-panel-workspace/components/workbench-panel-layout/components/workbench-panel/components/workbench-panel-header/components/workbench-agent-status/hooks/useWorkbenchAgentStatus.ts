import type { AgentLocationPresentation } from '@orchbee/agent';
import { useOrchbeeIntl } from '@orchbee/i18n';

import {
	formatWorkbenchAgentStatus,
	type WorkbenchAgentStatusView,
} from '../../../../../../../../../../../utils/formatWorkbenchAgentStatus';

export function useWorkbenchAgentStatus(
	presentation: AgentLocationPresentation,
): WorkbenchAgentStatusView {
	return formatWorkbenchAgentStatus(useOrchbeeIntl(), presentation);
}
