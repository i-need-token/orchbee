import { randomBytes } from 'node:crypto';
import { readFile } from 'node:fs/promises';
import path from 'node:path';

import { z } from 'zod';

import { writeStateRecordFile } from '../state-record';

const DESKTOP_TRAY_RECORD_VERSION = 1;

const DesktopTrayRecordSchema = z.strictObject({
	/** Fields a later release added, kept unread and written back unchanged. */
	extensions: z.record(z.string(), z.unknown()).optional(),
	noticeShown: z.boolean(),
	version: z.literal(DESKTOP_TRAY_RECORD_VERSION),
});

const VersionedRecordSchema = z.looseObject({ version: z.number().int() });

type DesktopTrayRecordExtensions = z.infer<typeof DesktopTrayRecordSchema>['extensions'];

/**
 * `internal/state/desktop-tray.json`, which only Desktop Main writes: whether the one-time notice
 * that Orchbee keeps running in the notification area has been shown. A missing, unreadable, or
 * malformed record shows the notice again, which costs one notice. A newer version's record means a
 * newer Orchbee showed it, so the notice stays hidden and the record is never rewritten.
 */
export class FileDesktopTrayRecord {
	#extensions: DesktopTrayRecordExtensions;
	readonly #filePath: string;
	#newer = false;
	readonly #randomToken: () => string;

	constructor(filePath: string, randomToken: () => string = () => randomBytes(16).toString('hex')) {
		this.#filePath = path.resolve(filePath);
		this.#randomToken = randomToken;
	}

	async readNoticeShown(): Promise<boolean> {
		let value: unknown;
		try {
			value = JSON.parse(await readFile(this.#filePath, 'utf8'));
		} catch {
			return false;
		}
		const record = DesktopTrayRecordSchema.safeParse(value);
		if (record.success) {
			this.#extensions = record.data.extensions;
			return record.data.noticeShown;
		}
		const versioned = VersionedRecordSchema.safeParse(value);
		if (versioned.success && versioned.data.version > DESKTOP_TRAY_RECORD_VERSION) {
			this.#newer = true;
			return true;
		}
		return false;
	}

	/** Records the notice as shown; a failed write only means a later run shows it again. */
	async recordNoticeShown(): Promise<void> {
		if (this.#newer) return;
		const record = {
			...(this.#extensions === undefined ? {} : { extensions: this.#extensions }),
			noticeShown: true,
			version: DESKTOP_TRAY_RECORD_VERSION,
		};
		await writeStateRecordFile(
			this.#filePath,
			Buffer.from(`${JSON.stringify(record)}\n`, 'utf8'),
			this.#randomToken,
		).catch(() => undefined);
	}
}
