import type { ActionExecutionContext, ActionResult } from '@orchbee/actions';

import type { WorkbenchAuthority, WorkbenchAuthorityClosure } from '../types/workbench-authority';
import type { WorkbenchTerminalHandover } from '../types/workbench-focus-routing';
import type { WorkbenchFocusAfterTabClose } from './workbench-tab-close-focus';

/**
 * Closes through the authority and gives the keyboard to the terminal left shown. As the TUI attaches
 * the neighbour before the close settles, `handover` completes once what closes leaves this Client's
 * view at the Server's acceptance, or else when the close completes. A Tab close first moves focus to
 * the Panel `focusAfterTabClose` picks, when it picks one, so that Panel takes the keyboard.
 */
export async function closeReturningToTerminal(
	authority: Pick<WorkbenchAuthority, 'close'>,
	context: ActionExecutionContext,
	closure: WorkbenchAuthorityClosure,
	handover: WorkbenchTerminalHandover | undefined,
	focusAfterTabClose?: WorkbenchFocusAfterTabClose | undefined,
): Promise<ActionResult<void>> {
	// Read before the close, while the focused Panel is still the one the person was in.
	const moveFocus = focusAfterTabClose?.prepare(context, closure) ?? null;
	let returned = false;
	const returnKeyboard = (): void => {
		if (returned) return;
		returned = true;
		// A keypress or click since the close began has already moved the keyboard, so focus stays.
		if (moveFocus !== null && handover?.held() === true && !context.signal.aborted) moveFocus();
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
