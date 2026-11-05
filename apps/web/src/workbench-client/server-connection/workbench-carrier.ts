/**
 * The OBWP frame around one Workbench request or response. `crates/workbench-protocol` owns each
 * frame kind, and `@orchbee/workbench-client-protocol` owns the values they carry.
 */
import {
	parseWorkbenchResponse,
	type WorkbenchRequest,
	type WorkbenchResponse,
} from '@orchbee/workbench-client-protocol';

const MAGIC = [0x4f, 0x42, 0x57, 0x50] as const; // OBWP
const VERSION = 1;
const HEADER_BYTES = 12;

type ClientViewRequest = Extract<WorkbenchRequest, { readonly kind: 'workbench.client-view' }>;
type ClientViewResponse = Extract<
	WorkbenchResponse,
	{ readonly kind: 'workbench.client-view.result' }
>;

const REQUEST_FRAME_KINDS: Readonly<
	Record<Exclude<WorkbenchRequest['kind'], ClientViewRequest['kind']>, number>
> = {
	'workbench.agent-integration': 63,
	'workbench.agent-manifests': 37,
	'workbench.close-operation.read': 61,
	'workbench.command': 2,
	'workbench.configuration': 39,
	'workbench.configured-command': 53,
	'workbench.current-directory': 65,
	'workbench.events': 55,
	'workbench.notification': 41,
	'workbench.pane-graphics': 57,
	'workbench.panel-agent': 35,
	'workbench.panel-metadata': 33,
	'workbench.plugin': 45,
	'workbench.publication.read': 9,
	'workbench.settings': 59,
	'workbench.snapshot.read': 1,
	'workbench.tab-close.read': 5,
	'workbench.tab-close.resume': 7,
	'workbench.tab-set-close': 67,
	'workbench.tab-duplicate': 69,
	'workbench.terminal-editor': 51,
	'workbench.window-title': 43,
	'workbench.workspace-close.read': 15,
	'workbench.workspace-close.resume': 17,
	'workbench.workspace-git': 49,
	'workbench.workspace-group-close': 23,
	'workbench.workspace-metadata': 31,
	'workbench.worktree-list.read': 11,
	'workbench.worktree.operation': 19,
	'workbench.worktree.query': 21,
};

const RESPONSE_FRAME_KINDS: Readonly<
	Record<Exclude<WorkbenchResponse['kind'], ClientViewResponse['kind']>, number>
> = {
	'workbench.agent-integration.result': 64,
	'workbench.agent-manifests.result': 38,
	'workbench.close-operation.result': 62,
	'workbench.command.result': 4,
	'workbench.configuration.result': 40,
	'workbench.configured-command.result': 54,
	'workbench.current-directory.result': 66,
	'workbench.events.result': 56,
	'workbench.notification.result': 42,
	'workbench.pane-graphics.result': 58,
	'workbench.panel-agent.result': 36,
	'workbench.panel-metadata.result': 34,
	'workbench.plugin.result': 46,
	'workbench.publication.result': 10,
	'workbench.settings.result': 60,
	'workbench.snapshot.result': 3,
	'workbench.tab-close.result': 6,
	'workbench.tab-close.resume.result': 8,
	'workbench.tab-set-close.result': 68,
	'workbench.tab-duplicate.result': 70,
	'workbench.terminal-editor.result': 52,
	'workbench.window-title.result': 44,
	'workbench.workspace-close.result': 16,
	'workbench.workspace-close.resume.result': 18,
	'workbench.workspace-git.result': 50,
	'workbench.workspace-group-close.result': 24,
	'workbench.workspace-metadata.result': 32,
	'workbench.worktree-list.result': 12,
	'workbench.worktree.operation.result': 20,
	'workbench.worktree.query.result': 22,
};

/** A Client View's frame kind follows what it carries: attention, a layout read, or neither. */
function clientViewRequestFrameKind({ action }: ClientViewRequest): number {
	const completion = action.kind === 'synchronize' ? action.completion?.outcome.kind : undefined;
	if (
		(action.kind === 'submit' &&
			(action.command.intent.kind === 'agent-attention' ||
				action.command.intent.kind === 'terminal-agent-attention')) ||
		completion === 'attention-read' ||
		completion === 'terminal-attention-read'
	) {
		return 29;
	}
	return completion === 'layout-read' ? 27 : 25;
}

function clientViewResponseFrameKind({ outcome }: ClientViewResponse): number {
	return outcome.kind === 'operation' && outcome.operation.status.kind === 'layout-read' ? 28 : 26;
}

function requestFrameKind(request: WorkbenchRequest): number {
	return request.kind === 'workbench.client-view'
		? clientViewRequestFrameKind(request)
		: REQUEST_FRAME_KINDS[request.kind];
}

function responseFrameKind(response: WorkbenchResponse): number {
	return response.kind === 'workbench.client-view.result'
		? clientViewResponseFrameKind(response)
		: RESPONSE_FRAME_KINDS[response.kind];
}

const utf8Encoder = new TextEncoder();
const utf8Decoder = new TextDecoder('utf-8', { fatal: true });

/** A Server of the same cohort never sends this, so it ends the connection. */
class WorkbenchCarrierError extends Error {
	constructor(message: string) {
		super(message);
		this.name = 'WorkbenchCarrierError';
	}
}

export function encodeWorkbenchRequest(request: WorkbenchRequest): Uint8Array {
	const payload = utf8Encoder.encode(JSON.stringify(request));
	const frame = new Uint8Array(HEADER_BYTES + payload.byteLength);
	const view = new DataView(frame.buffer);
	frame.set(MAGIC);
	view.setUint16(4, VERSION);
	view.setUint16(6, requestFrameKind(request));
	view.setUint32(8, payload.byteLength);
	frame.set(payload, HEADER_BYTES);
	return frame;
}

/** Decodes one complete response record received as a stream result. */
export function decodeWorkbenchResponse(record: Uint8Array): WorkbenchResponse {
	const view = new DataView(record.buffer, record.byteOffset, record.byteLength);
	if (
		record.byteLength < HEADER_BYTES ||
		MAGIC.some((byte, index) => record[index] !== byte) ||
		view.getUint16(4) !== VERSION ||
		record.byteLength !== HEADER_BYTES + view.getUint32(8)
	) {
		throw new WorkbenchCarrierError('Invalid OBWP response frame.');
	}
	const response = parseResponsePayload(record.subarray(HEADER_BYTES));
	if (responseFrameKind(response) !== view.getUint16(6)) {
		throw new WorkbenchCarrierError('OBWP frame kind does not match its response.');
	}
	return response;
}

function parseResponsePayload(payload: Uint8Array): WorkbenchResponse {
	try {
		return parseWorkbenchResponse(JSON.parse(utf8Decoder.decode(payload)));
	} catch {
		throw new WorkbenchCarrierError('Invalid OBWP response payload.');
	}
}
