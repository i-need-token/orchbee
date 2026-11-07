import type { ActionResult } from '@orchbee/actions';

import type {
	WorkbenchAuthority,
	WorkbenchAuthorityClosure,
	WorkbenchAuthorityContext,
} from '../types/workbench-authority';
import type { WorkbenchTerminalHandover } from '../types/workbench-focus-routing';

/**
 * Closes through the authority and gives the keyboard to the terminal left shown. As the TUI attaches
 * the neighbour before the close settles, `handover` completes once what closes leaves this Client's
 * view at the Server's acceptance, or else when the close completes.
 */
export async function closeReturningToTerminal(
	authority: Pick<WorkbenchAuthority, 'close'>,
	context: WorkbenchAuthorityContext,
	closure: WorkbenchAuthorityClosure,
	handover: WorkbenchTerminalHandover | undefined,
): Promise<ActionResult<void>> {
	let returned = false;
	const returnKeyboard = (): void => {
		if (returned) return;
		returned = true;
		handover?.complete();
	};
	const result = await authority.close(
		{
			markCommitted: () => context.markCommitted(),
			markLeft: returnKeyboard,
			signal: context.signal,
		},
		closure,
	);
	if (result.kind === 'completed') returnKeyboard();
	return result;
}
