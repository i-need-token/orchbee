import type { ServerConnection } from '../server-connection';
import {
	ServerPresentationRowSource,
	type ServerPanelAgentOutcome,
} from './ServerPresentationRowSource';
import type { ServerWorkbenchSession } from './ServerWorkbenchSession';
import type { ServerPresentationTitleRow } from './mergeServerTerminalTitle';
import { serverTerminalBindingKey } from './server-terminal-binding';

type PresentationTitleRow = Extract<
	ServerPanelAgentOutcome,
	{ readonly kind: 'presentation-titles' }
>['presentations'][number];

/** One Presentation's published title, with the Workbench binding the Server read it for. */
export interface ServerPresentationTitle extends ServerPresentationTitleRow {
	readonly presentationId: PresentationTitleRow['presentationId'];
}

/**
 * The titles the Server publishes for every bound Presentation in this session. A title that cannot
 * be read keeps whatever its Presentation last showed.
 */
export class ServerTerminalTitleSource extends ServerPresentationRowSource<PresentationTitleRow> {
	constructor(
		connection: Pick<ServerConnection, 'subscribe'>,
		session: Pick<ServerWorkbenchSession, 'onEnded' | 'snapshot' | 'subscribe'>,
		receive: (rows: readonly ServerPresentationTitle[]) => void,
	) {
		super(
			{
				action: (presentationIds) => ({ kind: 'presentation-titles', presentationIds }),
				// The most Presentations the Server answers in one `presentation-titles` request.
				limit: 64,
				rows: (outcome) => (outcome.kind === 'presentation-titles' ? outcome.presentations : null),
			},
			connection,
			session,
			(rows) =>
				receive(
					rows.map(({ binding, presentationId, title }) => ({
						binding: binding === null ? null : serverTerminalBindingKey(binding),
						presentationId,
						title,
					})),
				),
		);
	}
}
