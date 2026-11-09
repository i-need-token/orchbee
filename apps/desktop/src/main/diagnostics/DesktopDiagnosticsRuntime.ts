import { execFile } from 'node:child_process';
import path from 'node:path';

import { desktopDiagnosticsMessages, type IntlShape } from '@orchbee/i18n';
import { app, dialog, shell, type BrowserWindow } from 'electron';
import { z } from 'zod';

import type { DesktopDiagnosticRecorder } from './DesktopDiagnosticLog';
import type {
	DesktopDiagnosticBundleFailureCode,
	DesktopDiagnosticBundleOutcome,
	DesktopDiagnosticsTarget,
} from './DesktopDiagnosticsActions';

/** The command reads local files and asks a running Session once; this bounds a stalled process. */
const COMMAND_DEADLINE_MILLISECONDS = 120_000;

const DiagnosticsCreateDocumentSchema = z.looseObject({
	result: z.looseObject({ path: z.string().min(1) }),
	schema: z.literal('orchbee.diagnostics.create'),
});

/** The stages of a `write-failed` answer, under `orchbee.cli.failure.diagnostics`. */
const WRITE_FAILURE_STAGES = [
	'destination',
	'create-temporary',
	'write-temporary',
	'sync-temporary',
	'publish',
] as const satisfies readonly DesktopDiagnosticBundleFailureCode[];

const DiagnosticsFailureDocumentSchema = z.looseObject({
	failure: z.looseObject({
		details: z.looseObject({ stage: z.enum(WRITE_FAILURE_STAGES) }),
	}),
});

export interface DesktopDiagnosticsRuntimeOptions {
	readonly clientEnvironment: NodeJS.ProcessEnv;
	readonly clientExecutable: string;
	readonly intl: IntlShape;
	readonly recordEvent: DesktopDiagnosticRecorder;
	/** The window the save dialog belongs to, when one is open. */
	readonly window: () => BrowserWindow | undefined;
}

/** The Electron side of the Desktop diagnostics Action: the save dialog, the CLI, and the result. */
export class DesktopDiagnosticsRuntime implements DesktopDiagnosticsTarget {
	readonly #options: DesktopDiagnosticsRuntimeOptions;

	constructor(options: DesktopDiagnosticsRuntimeOptions) {
		this.#options = options;
	}

	async chooseBundlePath(): Promise<string | undefined> {
		const { intl } = this.#options;
		const options = {
			defaultPath: defaultBundlePath(new Date()),
			filters: [
				{ extensions: ['zip'], name: intl.formatMessage(desktopDiagnosticsMessages.zipFilter) },
			],
			properties: ['showOverwriteConfirmation' as const],
			title: intl.formatMessage(desktopDiagnosticsMessages.saveDialogTitle),
		};
		const window = this.#options.window();
		try {
			const selection =
				window === undefined
					? await dialog.showSaveDialog(options)
					: await dialog.showSaveDialog(window, options);
			return selection.canceled || selection.filePath === undefined || selection.filePath === ''
				? undefined
				: selection.filePath;
		} catch {
			return undefined;
		}
	}

	createBundle(bundlePath: string): Promise<DesktopDiagnosticBundleOutcome> {
		const { clientEnvironment, clientExecutable, recordEvent } = this.#options;
		return new Promise((resolve) => {
			execFile(
				clientExecutable,
				['diagnostics', 'create', '--output', bundlePath, '--format', 'json'],
				{ env: clientEnvironment, timeout: COMMAND_DEADLINE_MILLISECONDS, windowsHide: true },
				(error, stdout) => {
					const outcome = readOutcome(error, stdout);
					if (outcome.kind === 'created') recordEvent('info', 'diagnostic-bundle-created');
					else recordEvent('warning', 'diagnostic-bundle-failed', { code: outcome.code });
					resolve(outcome);
				},
			);
		});
	}

	async reportFailure(): Promise<void> {
		const { intl } = this.#options;
		const options = {
			buttons: [intl.formatMessage(desktopDiagnosticsMessages.close)],
			detail: intl.formatMessage(desktopDiagnosticsMessages.bundleFailedDetail),
			message: intl.formatMessage(desktopDiagnosticsMessages.bundleFailed),
			type: 'error' as const,
		};
		const window = this.#options.window();
		await (
			window === undefined ? dialog.showMessageBox(options) : dialog.showMessageBox(window, options)
		).catch(() => undefined);
	}

	revealBundle(bundlePath: string): void {
		shell.showItemInFolder(bundlePath);
	}
}

/**
 * The default name in the Desktop folder, or in the home folder when the system cannot name the
 * Desktop: Electron throws for a profile whose Desktop folder is missing, and the dialog still has to
 * open, in a folder that exists.
 */
function defaultBundlePath(now: Date): string {
	const name = defaultBundleName(now);
	for (const folder of ['desktop', 'home'] as const) {
		try {
			return path.join(app.getPath(folder), name);
		} catch {
			// The next folder, and at last the name alone, in whatever folder the dialog picks.
		}
	}
	return name;
}

/** `orchbee-diagnostics-YYYY-MM-DD-HHMM.zip` in local time. */
function defaultBundleName(now: Date): string {
	return `orchbee-diagnostics-${now.getFullYear()}-${two(now.getMonth() + 1)}-${two(now.getDate())}-${two(now.getHours())}${two(now.getMinutes())}.zip`;
}

function two(value: number): string {
	return String(value).padStart(2, '0');
}

/** The written path, or a closed failure code: the failed write's stage, never the failure text. */
function readOutcome(error: Error | null, stdout: string): DesktopDiagnosticBundleOutcome {
	let document: unknown;
	try {
		document = JSON.parse(stdout);
	} catch {
		return { code: error === null ? 'unreadable-result' : 'client-failed', kind: 'failed' };
	}
	if (error === null) {
		const created = DiagnosticsCreateDocumentSchema.safeParse(document);
		return created.success
			? { kind: 'created', path: created.data.result.path }
			: { code: 'unreadable-result', kind: 'failed' };
	}
	const failure = DiagnosticsFailureDocumentSchema.safeParse(document);
	return failure.success
		? { code: failure.data.failure.details.stage, kind: 'failed' }
		: { code: 'client-failed', kind: 'failed' };
}
