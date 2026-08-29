import type { WorkbenchProjectWorkspaceRuntime } from '../model/WorkbenchProjectWorkspaceRuntime';
import type {
	WorkbenchTerminalCreateOperationInput,
	WorkbenchTerminalCreateOperationResult,
} from './semantic-command-contracts';

export function executeWorkbenchTerminalCreate(
	runtime: WorkbenchProjectWorkspaceRuntime,
	input: WorkbenchTerminalCreateOperationInput,
	beforeCommit?: (() => void) | undefined,
): WorkbenchTerminalCreateOperationResult {
	return runtime.createTerminalAtLocation(input, beforeCommit);
}
