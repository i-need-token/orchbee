import {
	AttachmentIdSchema,
	CanonicalUint64TextSchema,
	CellWidth,
	encodeTerminalFrame,
	encodeTerminalGraphics,
	encodeTerminalRowWindow,
	materializeTerminalProjectionFrame,
	PROTOCOL_LIMITS,
	TERMINAL_PROJECTION_BUFFER_HEADER_BYTES,
	TERMINAL_ROW_WINDOW_HEADER_BYTES,
	TerminalFrameExtensionKind,
	TerminalRowWindowDetail,
	type AttachmentId,
	type DecodedTerminalProjectionBuffer,
	type SelectionChanged,
	type SelectionPoint,
	type TerminalFrame,
	type TerminalGraphics,
	type TerminalTextRow,
} from '@orchbee/terminal-protocol';
import {
	ProjectedAttachmentBootstrapSchema,
	ProjectedTerminalCommandSchema,
	ProjectedTerminalEventSchema,
	type ProjectedAttachmentBootstrap,
	type ProjectedTerminalCommand,
	type ProjectedTerminalEvent,
	type ProjectedTerminalFailure,
} from '@orchbee/workbench-client-protocol';
import type { TerminalPresentationId } from '@orchbee/workbench/terminal-presentation';
import type { MessagePortMain } from 'electron';

import type { ProjectedTerminalBrokerMetrics } from '../broker';
import type { DesktopTerminalProjectionLease } from '../terminal-projection-control';
import type { FileTerminalArchiveStore } from './FileTerminalArchiveStore';
import {
	terminalArchiveGraphicsForRows,
	terminalArchiveTextRows,
	type TerminalArchive,
} from './terminal-archive-document';

// Exact widths of the projection buffer's 16 columnar sections. This accounts admission only;
// encodeTerminalRowWindow remains the sole wire writer and the final size check fails closed.
const TERMINAL_PROJECTION_SECTION_ELEMENT_BYTES = [
	2, 1, 1, 4, 2, 4, 2, 4, 4, 4, 2, 1, 1, 1, 1, 1,
] as const;
const TERMINAL_TEXT_MAPPING_BYTES = 12;
const TERMINAL_TEXT_ROW_HEADER_BYTES = 12;
const textEncoder = new TextEncoder();

interface PreparedArchivedHyperlink {
	readonly byteLength: number;
	readonly byteValueKey: string;
}

export interface ArchivedTerminalProjectionServiceDependencies {
	readonly createMessageChannel: () => {
		readonly port1: MessagePortMain;
		readonly port2: MessagePortMain;
	};
	readonly randomAttachmentId: () => string;
	readonly store: FileTerminalArchiveStore;
	readonly writeClipboardText: (text: string) => Promise<void>;
}

export class ArchivedTerminalProjectionService {
	readonly #active = new Set<ArchivedTerminalBroker>();
	readonly #dependencies: ArchivedTerminalProjectionServiceDependencies;
	#disposed = false;

	constructor(dependencies: ArchivedTerminalProjectionServiceDependencies) {
		this.#dependencies = dependencies;
	}

	async open(
		presentationId: TerminalPresentationId,
	): Promise<DesktopTerminalProjectionLease | null> {
		if (this.#disposed) return null;
		const archive = await this.#dependencies.store.load(presentationId);
		if (archive === null || this.#disposed) return null;
		const attachmentId = AttachmentIdSchema.parse(this.#dependencies.randomAttachmentId());
		const bootstrap = ProjectedAttachmentBootstrapSchema.parse({
			attachmentId,
			canInput: false,
			canResize: false,
			capabilityRevision: CanonicalUint64TextSchema.parse('1'),
			creationId: archive.identity.creationId,
			hostEpoch: archive.identity.hostEpoch,
			terminalId: archive.identity.terminalId,
			terminalOutcome: null,
			theme: archive.theme,
		});
		const channel = this.#dependencies.createMessageChannel();
		let broker: ArchivedTerminalBroker;
		try {
			broker = new ArchivedTerminalBroker(
				archive,
				bootstrap,
				channel.port2,
				this.#dependencies.writeClipboardText,
				() => this.#active.delete(broker),
			);
		} catch (error) {
			closeChannel(channel);
			throw error;
		}
		this.#active.add(broker);
		broker.start();
		return {
			bootstrap,
			dispose: () => broker.dispose(),
			get metrics() {
				return broker.metrics;
			},
			rendererPort: channel.port1,
			setInputCapability: async (input) => {
				if (input) throw new Error('Archived terminal projections are permanently read-only.');
			},
		};
	}

	getMetrics(): readonly ProjectedTerminalBrokerMetrics[] {
		return [...this.#active].map((broker) => broker.metrics);
	}

	async dispose(): Promise<void> {
		if (this.#disposed) return;
		this.#disposed = true;
		const results = await Promise.allSettled(
			[...this.#active].map(async (broker) => broker.dispose()),
		);
		const failure = results.find(
			(result): result is PromiseRejectedResult => result.status === 'rejected',
		);
		if (failure !== undefined) throw failure.reason;
	}
}

class ArchivedTerminalBroker {
	readonly #archive: TerminalArchive;
	readonly #attachmentId: AttachmentId;
	#clipboardCommits = 0;
	#commandsAccepted = 0;
	#commandsRejected = 0;
	#committedViewportStart: bigint;
	#disposed = false;
	#frameBytesForwarded = 0;
	#framesForwarded = 0;
	readonly #onDispose: () => void;
	#operationTail = Promise.resolve();
	readonly #port: MessagePortMain;
	#rowWindowBytesForwarded = 0;
	#rowWindowsForwarded = 0;
	#selection: SelectionChanged;
	#sequence = 1n;
	readonly #sources: readonly DecodedTerminalProjectionBuffer[];
	#started = false;
	readonly #writeClipboardText: (text: string) => Promise<void>;

	constructor(
		archive: TerminalArchive,
		bootstrap: ProjectedAttachmentBootstrap,
		port: MessagePortMain,
		writeClipboardText: (text: string) => Promise<void>,
		onDispose: () => void,
	) {
		this.#archive = archive;
		this.#attachmentId = bootstrap.attachmentId;
		this.#committedViewportStart = BigInt(archive.retainedRows - archive.frame.rows);
		this.#onDispose = onDispose;
		this.#port = port;
		this.#selection = { attachmentId: this.#attachmentId, state: 'none' };
		this.#sources = [...archive.rowWindows.map((window) => window.frame), archive.frame];
		this.#writeClipboardText = writeClipboardText;
		const initial = this.#materializeFrame(
			this.#committedViewportStart,
			archive.frame.rows,
			this.#sequence,
		);
		if (!initial.ok) throw new Error(initial.detail);
	}

	get metrics(): ProjectedTerminalBrokerMetrics {
		return {
			clipboardCommits: this.#clipboardCommits,
			commandsAccepted: this.#commandsAccepted,
			commandsRejected: this.#commandsRejected,
			externalTargetsOpened: 0,
			frameBytesForwarded: this.#frameBytesForwarded,
			framesForwarded: this.#framesForwarded,
			lastSelectionCopyMilliseconds: null,
			lastSelectionExportStartMilliseconds: null,
			rowWindowBytesForwarded: this.#rowWindowBytesForwarded,
			rowWindowsForwarded: this.#rowWindowsForwarded,
			selectionExportBytesForwarded: 0,
			selectionExportChunksForwarded: 0,
		};
	}

	start(): void {
		if (this.#started || this.#disposed) return;
		this.#started = true;
		this.#port.on('message', this.#handleMessage);
		this.#port.on('close', this.#handleClose);
		this.#port.start();
	}

	dispose(): Promise<void> {
		if (this.#disposed) return Promise.resolve();
		this.#disposed = true;
		if (this.#started) {
			this.#port.off('message', this.#handleMessage);
			this.#port.off('close', this.#handleClose);
		}
		try {
			this.#port.close();
		} finally {
			this.#onDispose();
		}
		return Promise.resolve();
	}

	readonly #handleClose = (): void => {
		void this.dispose();
	};

	readonly #handleMessage = (message: { readonly data: unknown }): void => {
		if (this.#disposed) return;
		const parsed = ProjectedTerminalCommandSchema.safeParse(message.data);
		if (!parsed.success) {
			this.#commandsRejected += 1;
			void this.dispose();
			return;
		}
		this.#commandsAccepted += 1;
		const operation = this.#operationTail.then(async () => this.#dispatch(parsed.data));
		this.#operationTail = operation.catch((error: unknown) => {
			this.#commandsRejected += 1;
			this.#postFailure(parsed.data, error);
		});
	};

	async #dispatch(command: ProjectedTerminalCommand): Promise<void> {
		if (this.#disposed) return;
		if (command.payload.attachmentId !== this.#attachmentId) {
			throw new Error('Archived terminal command names a foreign attachment.');
		}
		switch (command.kind) {
			case 'set-projection-demand':
				if (command.payload.demand === 'active') this.#postFrame();
				return;
			case 'need-snapshot':
				this.#postFrame();
				return;
			case 'read-rows':
				this.#postRows(command);
				return;
			case 'selection-command':
				this.#applySelectionCommand(command);
				return;
			case 'selection-gesture':
				this.#applySelectionGesture(command);
				return;
			case 'selection-copy':
				await this.#copySelection(command.payload.requestSequence);
				return;
			case 'reconcile-terminal-theme':
				this.#post({
					current: this.#archive.theme,
					kind: 'terminal-theme-reconciled',
					operationId: command.payload.operationId,
					terminalId: this.#archive.identity.terminalId,
				});
				return;
			case 'apply-terminal-theme':
				this.#post({
					failure: {
						code: 'terminal-archive-read-only',
						outcome: 'known-unchanged',
						retryable: false,
					},
					kind: 'terminal-theme-failed',
					operationId: command.payload.operationId,
				});
				return;
			case 'frame-applied':
			case 'history-observed':
			case 'rows-applied':
			case 'set-history-demand':
				return;
			case 'clear-terminal':
			case 'clipboard-write-decision':
			case 'focus-input':
			case 'key-input':
			case 'link-open':
			case 'mouse-input':
			case 'paste-input':
			case 'reset-terminal':
			case 'resize-terminal':
			case 'text-input':
				throw new Error('Archived terminal projections are read-only.');
		}
	}

	#postFrame(): void {
		this.#sequence += 1n;
		const encoded = this.#materializeFrame(
			this.#committedViewportStart,
			this.#archive.frame.rows,
			this.#sequence,
		);
		if (!encoded.ok) throw new Error(encoded.detail);
		const graphics = terminalArchiveGraphicsForRows(
			this.#archive,
			this.#committedViewportStart,
			this.#archive.frame.rows,
		);
		if (graphics === null) throw new Error('Archived terminal graphics cannot be materialized.');
		let materializedFrame = encoded.value;
		if (graphics.images.length > 0 || graphics.placements.length > 0) {
			const graphicsBytes = encodeTerminalGraphics(graphics);
			if (!graphicsBytes.ok) throw new Error(graphicsBytes.detail);
			const extensionData = new Uint8Array(graphicsBytes.value.byteLength);
			extensionData.set(graphicsBytes.value);
			materializedFrame = {
				...materializedFrame,
				extensions: [
					...materializedFrame.extensions.filter(
						(extension) => extension.kind !== TerminalFrameExtensionKind.kittyGraphics,
					),
					{
						data: extensionData,
						kind: TerminalFrameExtensionKind.kittyGraphics,
						required: false,
					},
				],
			};
		}
		const frame = encodeTerminalFrame(materializedFrame);
		if (!frame.ok) throw new Error(frame.detail);
		this.#frameBytesForwarded += frame.value.byteLength;
		this.#framesForwarded += 1;
		this.#post({ frame: exactArrayBuffer(frame.value), kind: 'frame' });
	}

	#postRows(command: Extract<ProjectedTerminalCommand, { readonly kind: 'read-rows' }>): void {
		this.#assertRevision(command.payload.generation, command.payload.revision);
		const resolved = this.#resolveRowRequest(command);
		const minimumRows =
			command.payload.purpose === 'viewport'
				? Math.min(this.#archive.frame.rows, resolved.maximumRows)
				: 1;
		const maximumMaterializableRows = Math.min(
			PROTOCOL_LIMITS.maxFrameRows,
			Math.floor(PROTOCOL_LIMITS.maxFrameCells / this.#archive.frame.columns),
		);
		const maximumRows = Math.min(
			command.payload.rowCount,
			resolved.maximumRows,
			maximumMaterializableRows,
		);
		const materialized =
			maximumRows >= minimumRows
				? this.#materializeFrame(
						resolved.startRow,
						maximumRows,
						BigInt(command.payload.requestSequence),
					)
				: null;
		const graphics =
			materialized?.ok === true && command.payload.detail === TerminalRowWindowDetail.render
				? terminalArchiveGraphicsForRows(this.#archive, resolved.startRow, maximumRows)
				: null;
		const encoded =
			materialized?.ok === true &&
			(command.payload.detail !== TerminalRowWindowDetail.render || graphics !== null)
				? encodeLargestArchivedRowWindow(
						command.payload.detail,
						materialized.value,
						graphics,
						{
							attachmentId: this.#attachmentId,
							columns: this.#archive.frame.columns,
							generation: this.#archive.frame.generation,
							hostEpoch: this.#archive.frame.hostEpoch,
							revision: this.#archive.frame.revision,
							sequence: BigInt(command.payload.requestSequence),
							startRow: resolved.startRow,
							totalRows: BigInt(this.#archive.retainedRows),
						},
						minimumRows,
						command.payload.maximumBytes,
					)
				: null;
		if (encoded === null) throw new Error('Archived terminal row window exceeds its byte limit.');
		if (command.payload.purpose === 'viewport') {
			this.#committedViewportStart = resolved.startRow;
		}
		this.#rowWindowBytesForwarded += encoded.byteLength;
		this.#rowWindowsForwarded += 1;
		this.#post({ kind: 'rows', rows: exactArrayBuffer(encoded) });
	}

	#resolveRowRequest(command: Extract<ProjectedTerminalCommand, { readonly kind: 'read-rows' }>): {
		readonly maximumRows: number;
		readonly startRow: bigint;
	} {
		const totalRows = BigInt(this.#archive.retainedRows);
		const origin = command.payload.origin;
		let startRow: bigint;
		let endRow = totalRows;
		switch (origin.kind) {
			case 'absolute':
				startRow = BigInt(origin.row);
				break;
			case 'oldest':
				startRow = BigInt(origin.offset);
				break;
			case 'committed': {
				const requested = this.#committedViewportStart + BigInt(origin.delta);
				const maximumStart =
					totalRows > BigInt(this.#archive.frame.rows)
						? totalRows - BigInt(this.#archive.frame.rows)
						: 0n;
				startRow = requested;
				if (startRow < 0n) startRow = 0n;
				if (startRow > maximumStart) startRow = maximumStart;
				break;
			}
			case 'tail':
				endRow =
					BigInt(origin.rowsBefore) >= totalRows ? 0n : totalRows - BigInt(origin.rowsBefore);
				startRow =
					endRow > BigInt(command.payload.rowCount)
						? endRow - BigInt(command.payload.rowCount)
						: 0n;
				break;
		}
		if (startRow < 0n || startRow >= totalRows || endRow <= startRow) {
			throw new Error('Archived terminal row request is outside the retained range.');
		}
		return {
			maximumRows: Number(endRow - startRow),
			startRow,
		};
	}

	#applySelectionCommand(
		command: Extract<ProjectedTerminalCommand, { readonly kind: 'selection-command' }>,
	): void {
		this.#assertRevision(command.payload.generation, command.payload.revision);
		switch (command.payload.command) {
			case 'clear':
				this.#selection = { attachmentId: this.#attachmentId, state: 'none' };
				break;
			case 'select-all':
				this.#selection = this.#activeSelection(
					{ column: 0, row: CanonicalUint64TextSchema.parse('0') },
					{
						column: this.#archive.frame.columns - 1,
						row: CanonicalUint64TextSchema.parse((this.#archive.retainedRows - 1).toString()),
					},
					false,
				);
				break;
			case 'set-range':
				this.#selection = this.#activeSelection(
					command.payload.start,
					command.payload.end,
					command.payload.rectangle,
				);
				break;
			case 'select-output':
				this.#selection = this.#activeSelection(
					{ column: 0, row: command.payload.point.row },
					{
						column: this.#archive.frame.columns - 1,
						row: command.payload.point.row,
					},
					false,
				);
				break;
		}
		this.#post({ event: this.#selection, kind: 'selection-changed' });
	}

	#applySelectionGesture(
		command: Extract<ProjectedTerminalCommand, { readonly kind: 'selection-gesture' }>,
	): void {
		this.#assertRevision(command.payload.generation, command.payload.revision);
		if (command.payload.action === 'cancel') {
			this.#selection = { attachmentId: this.#attachmentId, state: 'none' };
		} else if (command.payload.action === 'start') {
			this.#selection = command.payload.selectOutput
				? this.#activeSelection(
						{ column: 0, row: command.payload.point.row },
						{
							column: this.#archive.frame.columns - 1,
							row: command.payload.point.row,
						},
						false,
					)
				: this.#activeSelection(command.payload.point, command.payload.point, false);
		} else if (this.#selection.state === 'active') {
			this.#selection = this.#activeSelection(
				this.#selection.start,
				command.payload.point,
				command.payload.action === 'update' ? command.payload.rectangle : this.#selection.rectangle,
			);
		}
		this.#post({ event: this.#selection, kind: 'selection-changed' });
	}

	#activeSelection(
		start: SelectionPoint,
		end: SelectionPoint,
		rectangle: boolean,
	): Extract<SelectionChanged, { readonly state: 'active' }> {
		this.#assertSelectionPoint(start);
		this.#assertSelectionPoint(end);
		return {
			attachmentId: this.#attachmentId,
			end,
			preview: null,
			rectangle,
			start,
			state: 'active',
		};
	}

	async #copySelection(
		requestSequence: Extract<
			ProjectedTerminalCommand,
			{ readonly kind: 'selection-copy' }
		>['payload']['requestSequence'],
	): Promise<void> {
		if (this.#selection.state !== 'active') {
			this.#post({ byteLength: 0, kind: 'selection-copied', requestSequence });
			return;
		}
		const text = this.#selectionText(this.#selection);
		const bytes = new TextEncoder().encode(text);
		if (bytes.byteLength > PROTOCOL_LIMITS.maxSelectionExportBytes) {
			throw new Error('Archived terminal selection exceeds the copy limit.');
		}
		await this.#writeClipboardText(text);
		this.#clipboardCommits += 1;
		this.#post({ byteLength: bytes.byteLength, kind: 'selection-copied', requestSequence });
	}

	#selectionText(selection: Extract<SelectionChanged, { readonly state: 'active' }>): string {
		const [start, end] = orderedPoints(selection.start, selection.end);
		const output: string[] = [];
		let row = BigInt(start.row);
		const lastRow = BigInt(end.row);
		const maximumChunkRows = Math.max(
			1,
			Math.min(
				PROTOCOL_LIMITS.maxFrameRows,
				Math.floor(PROTOCOL_LIMITS.maxFrameCells / this.#archive.frame.columns),
			),
		);
		while (row <= lastRow) {
			const rowCount = Number(minimumBigInt(BigInt(maximumChunkRows), lastRow - row + 1n));
			const materialized = this.#materializeFrame(row, rowCount, 1n);
			if (!materialized.ok) throw new Error(materialized.detail);
			for (const [offset, rowData] of materialized.value.rowData.entries()) {
				const absoluteRow = row + BigInt(offset);
				const firstColumn =
					selection.rectangle || absoluteRow === BigInt(start.row) ? start.column : 0;
				const lastColumn =
					selection.rectangle || absoluteRow === BigInt(end.row)
						? end.column
						: this.#archive.frame.columns - 1;
				output.push(rowText(rowData, firstColumn, lastColumn));
				if (absoluteRow < lastRow && (selection.rectangle || !rowData.wrap)) output.push('\n');
			}
			row += BigInt(rowCount);
		}
		return output.join('');
	}

	#assertSelectionPoint(point: SelectionPoint): void {
		if (
			point.column >= this.#archive.frame.columns ||
			BigInt(point.row) >= BigInt(this.#archive.retainedRows)
		) {
			throw new Error('Archived terminal selection is outside the retained range.');
		}
	}

	#assertRevision(generation: string, revision: string): void {
		if (
			BigInt(generation) !== this.#archive.frame.generation ||
			BigInt(revision) !== this.#archive.frame.revision
		) {
			throw new Error('Archived terminal command used a stale projection revision.');
		}
	}

	#materializeFrame(startRow: bigint, rowCount: number, sequence: bigint) {
		return materializeTerminalProjectionFrame(this.#archive.frame, this.#sources, {
			attachmentId: this.#attachmentId,
			revealCursor: false,
			rowCount,
			scrollbackRows: startRow,
			sequence,
			sourceStartRow: this.#archive.sourceStartRow + startRow,
		});
	}

	#post(event: ProjectedTerminalEvent): void {
		if (this.#disposed) return;
		const parsed = ProjectedTerminalEventSchema.safeParse(event);
		if (!parsed.success) throw new Error('Archived terminal produced an invalid projection event.');
		this.#port.postMessage(parsed.data);
	}

	#postFailure(command: ProjectedTerminalCommand, error: unknown): void {
		if (this.#disposed) return;
		const context = failureContext(command);
		const failure: ProjectedTerminalFailure = {
			code: 'terminal-archive-read-only',
			message: error instanceof Error ? error.message : 'Archived terminal command failed.',
			...context,
			retryable: false,
		};
		try {
			this.#post({ failure, kind: 'failure' });
		} catch {
			void this.dispose();
		}
	}
}

interface ArchivedRowWindowMetadata {
	readonly attachmentId: AttachmentId;
	readonly columns: number;
	readonly generation: bigint;
	readonly hostEpoch: TerminalFrame['hostEpoch'];
	readonly revision: bigint;
	readonly sequence: bigint;
	readonly startRow: bigint;
	readonly totalRows: bigint;
}

function encodeLargestArchivedRowWindow(
	detail: TerminalRowWindowDetail,
	frame: TerminalFrame,
	graphics: TerminalGraphics | null,
	metadata: ArchivedRowWindowMetadata,
	minimumRows: number,
	maximumBytes: number,
): Uint8Array | null {
	if (detail === TerminalRowWindowDetail.render) {
		if (graphics === null) return null;
		const maximumRows = maximumEncodableRenderRows(frame, minimumRows, maximumBytes);
		if (maximumRows === null) return null;
		const encodeRows = (rowCount: number) => {
			const candidateFrame =
				rowCount === frame.rows
					? frame
					: { ...frame, rowData: frame.rowData.slice(0, rowCount), rows: rowCount };
			return encodeTerminalRowWindow({
				...metadata,
				detail: TerminalRowWindowDetail.render,
				frame: candidateFrame,
				graphics: graphicsForLeadingRows(graphics, rowCount, frame.cellPixelHeight),
			});
		};
		const full = encodeRows(maximumRows);
		if (full.ok && full.value.byteLength <= maximumBytes) return full.value;
		let lower = minimumRows;
		let upper = maximumRows - 1;
		let largest: Uint8Array | null = null;
		while (lower <= upper) {
			const rowCount = Math.floor((lower + upper) / 2);
			const encoded = encodeRows(rowCount);
			if (encoded.ok && encoded.value.byteLength <= maximumBytes) {
				largest = encoded.value;
				lower = rowCount + 1;
			} else {
				upper = rowCount - 1;
			}
		}
		return largest;
	}

	const rows = terminalArchiveTextRows(frame, metadata.startRow);
	const rowCount = maximumEncodableTextRows(rows, minimumRows, maximumBytes);
	if (rowCount === null) return null;
	const encoded = encodeTerminalRowWindow({
		...metadata,
		detail: TerminalRowWindowDetail.text,
		rows: rows.slice(0, rowCount),
	});
	return encoded.ok && encoded.value.byteLength <= maximumBytes ? encoded.value : null;
}

function graphicsForLeadingRows(
	graphics: TerminalGraphics,
	rowCount: number,
	cellPixelHeight: number,
): TerminalGraphics {
	const maximumY = rowCount * cellPixelHeight;
	const placements = graphics.placements.filter((placement) => {
		const top = placement.row * cellPixelHeight + placement.offsetY;
		return top < maximumY && top + placement.destinationHeight > 0;
	});
	const imageIds = new Set(placements.map((placement) => placement.imageId));
	return {
		...graphics,
		images: graphics.images.filter((image) => imageIds.has(image.id)),
		placements,
	};
}

function maximumEncodableRenderRows(
	frame: TerminalFrame,
	minimumRows: number,
	maximumBytes: number,
): number | null {
	const stringByteLengths = new Map<string, number>();
	const preparedHyperlinksByStyle = new Map<number, PreparedArchivedHyperlink | null>();
	const admittedHyperlinkByteValues = new Set<string>();
	let stringBytes =
		utf8ByteLength(frame.title, stringByteLengths) +
		utf8ByteLength(frame.currentDirectory, stringByteLengths);
	let maximumRows = 0;
	for (const [rowOffset, row] of frame.rowData.entries()) {
		for (const cell of row.cells) {
			const style = frame.styles[cell.style];
			if (style === undefined) return null;
			stringBytes += utf8ByteLength(cell.grapheme, stringByteLengths);
			let preparedHyperlink = preparedHyperlinksByStyle.get(cell.style);
			if (!preparedHyperlinksByStyle.has(cell.style)) {
				if (style.hyperlink === null) {
					preparedHyperlink = null;
				} else {
					const hyperlinkBytes = textEncoder.encode(style.hyperlink);
					preparedHyperlink = {
						byteLength: hyperlinkBytes.byteLength,
						byteValueKey: projectionByteValueKey(hyperlinkBytes),
					};
				}
				preparedHyperlinksByStyle.set(cell.style, preparedHyperlink);
			}
			if (preparedHyperlink !== null && preparedHyperlink !== undefined) {
				if (!admittedHyperlinkByteValues.has(preparedHyperlink.byteValueKey)) {
					admittedHyperlinkByteValues.add(preparedHyperlink.byteValueKey);
					stringBytes += preparedHyperlink.byteLength;
				}
			}
		}
		const rowCount = rowOffset + 1;
		const byteLength =
			TERMINAL_ROW_WINDOW_HEADER_BYTES +
			terminalProjectionBufferByteLength(rowCount, frame.columns, stringBytes);
		if (byteLength > maximumBytes) break;
		maximumRows = rowCount;
	}
	return maximumRows >= minimumRows ? maximumRows : null;
}

function maximumEncodableTextRows(
	rows: readonly TerminalTextRow[],
	minimumRows: number,
	maximumBytes: number,
): number | null {
	let byteLength = TERMINAL_ROW_WINDOW_HEADER_BYTES;
	let maximumRows = 0;
	for (const [rowOffset, row] of rows.entries()) {
		byteLength += TERMINAL_TEXT_ROW_HEADER_BYTES + textEncoder.encode(row.text).byteLength;
		if (!usesIdentityAsciiMapping(row)) {
			byteLength += row.mappings.length * TERMINAL_TEXT_MAPPING_BYTES;
		}
		if (byteLength > maximumBytes) break;
		maximumRows = rowOffset + 1;
	}
	return maximumRows >= minimumRows ? maximumRows : null;
}

function terminalProjectionBufferByteLength(
	rowCount: number,
	columns: number,
	stringByteLength: number,
): number {
	const cellCount = rowCount * columns;
	let byteLength = TERMINAL_PROJECTION_BUFFER_HEADER_BYTES;
	for (const [section, elementBytes] of TERMINAL_PROJECTION_SECTION_ELEMENT_BYTES.entries()) {
		const remainder = byteLength % elementBytes;
		if (remainder !== 0) byteLength += elementBytes - remainder;
		let elementCount = cellCount;
		if (section <= 2) elementCount = rowCount;
		else if (section === 15) elementCount = stringByteLength;
		byteLength += elementCount * elementBytes;
	}
	return byteLength;
}

function utf8ByteLength(value: string | null, cache: Map<string, number>): number {
	if (value === null) return 0;
	const cached = cache.get(value);
	if (cached !== undefined) return cached;
	const byteLength = textEncoder.encode(value).byteLength;
	cache.set(value, byteLength);
	return byteLength;
}

function projectionByteValueKey(bytes: Uint8Array): string {
	const chunks: string[] = [];
	for (let offset = 0; offset < bytes.byteLength; offset += 4_096) {
		chunks.push(String.fromCharCode(...bytes.subarray(offset, offset + 4_096)));
	}
	return chunks.join('');
}

function usesIdentityAsciiMapping(row: TerminalTextRow): boolean {
	if (row.mappings.length !== row.text.length) return false;
	for (let index = 0; index < row.text.length; index += 1) {
		const mapping = row.mappings[index];
		if (
			row.text.charCodeAt(index) > 0x7f ||
			mapping === undefined ||
			mapping.byteOffset !== index ||
			mapping.utf16Offset !== index ||
			mapping.column !== index ||
			mapping.width !== 1
		) {
			return false;
		}
	}
	return true;
}

function rowText(row: TerminalFrame['rowData'][number], startColumn: number, endColumn: number) {
	let text = '';
	for (let column = startColumn; column <= endColumn; column += 1) {
		const cell = row.cells[column];
		if (cell === undefined || cell.width === CellWidth.spacerTail) continue;
		text += cell.grapheme;
	}
	return text.replace(/[\t ]+$/u, '');
}

function orderedPoints(
	left: SelectionPoint,
	right: SelectionPoint,
): readonly [SelectionPoint, SelectionPoint] {
	const leftRow = BigInt(left.row);
	const rightRow = BigInt(right.row);
	return leftRow < rightRow || (leftRow === rightRow && left.column <= right.column)
		? [left, right]
		: [right, left];
}

function failureContext(
	command: ProjectedTerminalCommand,
): Pick<ProjectedTerminalFailure, 'operation' | 'requestSequence'> {
	switch (command.kind) {
		case 'read-rows':
			return { operation: 'history', requestSequence: command.payload.requestSequence };
		case 'selection-copy':
			return { operation: 'copy', requestSequence: command.payload.requestSequence };
		case 'selection-command':
		case 'selection-gesture':
			return { operation: 'selection' };
		case 'focus-input':
		case 'key-input':
		case 'mouse-input':
		case 'paste-input':
		case 'text-input':
			return { operation: 'input' };
		case 'resize-terminal':
			return { operation: 'resize' };
		case 'link-open':
			return { operation: 'link', requestSequence: command.payload.requestSequence };
		case 'clipboard-write-decision':
			return { operation: 'clipboard', requestSequence: command.payload.sequence };
		case 'clear-terminal':
		case 'reset-terminal':
			return { operation: 'control' };
		case 'frame-applied':
		case 'history-observed':
		case 'need-snapshot':
		case 'rows-applied':
		case 'set-history-demand':
		case 'set-projection-demand':
		case 'apply-terminal-theme':
		case 'reconcile-terminal-theme':
			return { operation: 'protocol' };
	}
}

function exactArrayBuffer(bytes: Uint8Array): ArrayBuffer {
	const copy = new Uint8Array(bytes.byteLength);
	copy.set(bytes);
	return copy.buffer;
}

function minimumBigInt(left: bigint, right: bigint): bigint {
	return left < right ? left : right;
}

function closeChannel(channel: {
	readonly port1: MessagePortMain;
	readonly port2: MessagePortMain;
}): void {
	try {
		channel.port1.close();
	} catch {
		// A failed archive open owns no durable resource beyond best-effort port closure.
	}
	try {
		channel.port2.close();
	} catch {
		// A failed archive open owns no durable resource beyond best-effort port closure.
	}
}
