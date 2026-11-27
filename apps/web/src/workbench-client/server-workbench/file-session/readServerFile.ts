import type { FileContent, FileReadOptions, FileResult } from '../../../file-session';
import { FILES_OPERATIONS } from '../../server-connection';
import type { ServerFileChannel } from './ServerFileChannel';
import { readServerTransfer } from './readServerTransfer';

/** Reads one file of the Workspace through `fs.read`. */
export function readServerFile(
	channel: ServerFileChannel,
	workspaceId: string,
	request: FileReadOptions & { readonly path: string },
	signal: AbortSignal,
): Promise<FileResult<FileContent>> {
	return readServerTransfer(channel, workspaceId, request, signal, (transfer) =>
		FILES_OPERATIONS['fs.read'].encode({
			encoding: request.encoding,
			forceText: request.forceText,
			limit: request.limit,
			mode: request.mode,
			op: 'fs.read',
			path: request.path,
			transfer,
			workspaceId,
		}),
	);
}
