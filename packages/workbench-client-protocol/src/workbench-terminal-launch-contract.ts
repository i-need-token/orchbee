import { z } from 'zod';
import { WorkbenchDirectoryPathSchema } from './workbench-semantic-contract';

export const WORKBENCH_MAXIMUM_ENCODED_LAUNCH_COMMAND_BYTES = 96 * 1024;

const literalLaunchStringSchema = z
	.string()
	.refine(
		(value) => !value.includes('\0') && !/[\uD800-\uDFFF]/u.test(value),
		'Launch text must be NUL-free Unicode scalar text',
	);
const utf8 = new TextEncoder();
export const WorkbenchLaunchSelectionSchema = z
	.strictObject({
		exec: literalLaunchStringSchema.refine(
			(value) => /^(?:\/|(?:\\\\\?\\)?[A-Za-z]:[/\\])/u.test(value),
			'Launch executable must be an absolute path',
		),
		args: z.array(literalLaunchStringSchema).max(128).readonly(),
	})
	.refine((value) => {
		let bytes = 0;
		let escaped = 0;
		for (const text of [value.exec, ...value.args]) {
			for (const byte of utf8.encode(text)) {
				bytes += 1;
				if (byte <= 31) escaped += 6;
				else if (byte === 34 || byte === 92) escaped += 2;
				else escaped += 1;
			}
		}
		return bytes <= 30 * 1024 && escaped <= 30 * 1024;
	}, 'Launch selection exceeds its byte budget')
	.readonly();

export const WorkbenchLaunchProfileSelectorSchema = z
	.string()
	.min(1)
	.max(512)
	.refine(
		(value) => !/[\uD800-\uDFFF]/u.test(value) && utf8.encode(value).length <= 512,
		'Launch profile selector exceeds its UTF-8 bound',
	);

const environmentNameSchema = z
	.string()
	.regex(/^[A-Za-z_][A-Za-z0-9_]{0,127}$/u)
	.refine(
		(value) =>
			!value.toUpperCase().startsWith('ORCHBEE_') &&
			!['TERM', 'COLORTERM', 'TERM_PROGRAM'].includes(value.toUpperCase()),
		'Launch environment cannot override a managed name',
	);
const pluginPaneSelectorSchema = z
	.strictObject({
		pluginId: z.string().regex(/^[A-Za-z0-9._:-]{1,120}$/u),
		entrypointId: z.string().regex(/^[A-Za-z0-9_:-]{1,120}$/u),
	})
	.readonly();
export const WorkbenchLaunchOptionsSchema = z
	.strictObject({
		cwd: WorkbenchDirectoryPathSchema.nullable().optional(),
		pluginPane: pluginPaneSelectorSchema.nullable().optional(),
		environment: z
			.array(z.tuple([environmentNameSchema, literalLaunchStringSchema]).readonly())
			.max(32)
			.refine((entries) => {
				const names = new Set<string>();
				let bytes = 0;
				for (const [name, value] of entries) {
					const key = name.toUpperCase();
					if (names.has(key)) return false;
					names.add(key);
					bytes += utf8.encode(name).length + utf8.encode(value).length;
				}
				return bytes <= 1024;
			}, 'Launch environment has duplicate names or exceeds its byte budget')
			.readonly()
			.optional(),
	})
	.readonly();

export const WorkbenchTerminalLaunchFieldsSchema = z
	.strictObject({
		launch: WorkbenchLaunchSelectionSchema.nullable().optional(),
		profile: WorkbenchLaunchProfileSelectorSchema.nullable().optional(),
		options: WorkbenchLaunchOptionsSchema.optional(),
	})
	.refine(
		(value) =>
			value.launch === null ||
			value.launch === undefined ||
			value.profile === null ||
			value.profile === undefined,
		'Launch and profile are mutually exclusive',
	)
	.refine(
		(value) => !value.options?.pluginPane || (!value.launch && !value.profile),
		'Plugin Pane launch excludes executable/profile',
	);

type TerminalLaunchFields = z.output<typeof WorkbenchTerminalLaunchFieldsSchema>;
type CanonicalTerminalLaunchFields = Readonly<{
	launch?: z.output<typeof WorkbenchLaunchSelectionSchema>;
	profile?: z.output<typeof WorkbenchLaunchProfileSelectorSchema>;
	options?: Readonly<{
		cwd?: z.output<typeof WorkbenchDirectoryPathSchema>;
		pluginPane?: z.output<typeof pluginPaneSelectorSchema>;
		environment?: NonNullable<z.output<typeof WorkbenchLaunchOptionsSchema>['environment']>;
	}>;
}>;

/** Normalize optional launch fields exactly as the native serializer does. */
export function normalizeWorkbenchTerminalLaunch<T extends TerminalLaunchFields>(
	value: T,
): Omit<T, keyof TerminalLaunchFields> & CanonicalTerminalLaunchFields {
	const { launch, profile, options, ...action } = value;
	return {
		...action,
		...(launch ? { launch } : {}),
		...(profile ? { profile } : {}),
		...(options && (options.cwd || options.environment?.length || options.pluginPane)
			? {
					options: {
						...(options.cwd ? { cwd: options.cwd } : {}),
						...(options.pluginPane ? { pluginPane: options.pluginPane } : {}),
						...(options.environment?.length ? { environment: options.environment } : {}),
					},
				}
			: {}),
	};
}

const terminalGeometryDimensionSchema = z.number().int().min(1).max(4096);
const terminalCellPixelDimensionSchema = z.number().int().min(0).max(4096);

export const WorkbenchTerminalGeometrySchema = z
	.strictObject({
		columns: terminalGeometryDimensionSchema,
		rows: terminalGeometryDimensionSchema,
		cellPixelWidth: terminalCellPixelDimensionSchema,
		cellPixelHeight: terminalCellPixelDimensionSchema,
	})
	.superRefine((geometry, context) => {
		if ((geometry.cellPixelWidth === 0) !== (geometry.cellPixelHeight === 0)) {
			context.addIssue({
				code: 'custom',
				message:
					'Workbench terminal cell pixel width and height must be both positive or both zero (pixels unknown)',
				path: ['cellPixelWidth'],
			});
		}
		if (geometry.columns * geometry.rows > 262_144) {
			context.addIssue({
				code: 'custom',
				message: 'Workbench terminal grid exceeds 262144 cells',
				path: ['columns'],
			});
		}
		if (geometry.columns * geometry.cellPixelWidth > 65_535) {
			context.addIssue({
				code: 'custom',
				message: 'Workbench terminal pixel width exceeds 65535',
				path: ['cellPixelWidth'],
			});
		}
		if (geometry.rows * geometry.cellPixelHeight > 65_535) {
			context.addIssue({
				code: 'custom',
				message: 'Workbench terminal pixel height exceeds 65535',
				path: ['cellPixelHeight'],
			});
		}
	})
	.readonly();

export const WorkbenchTerminalUnavailableReasonSchema = z.enum([
	'terminal-runtime-unavailable',
	'server-busy',
]);
export const WorkbenchTerminalFailureReasonSchema = z.enum(['terminal-create-failed']);
export const WorkbenchTerminalOutcomeSchema = z.discriminatedUnion('kind', [
	z
		.strictObject({ kind: z.literal('started'), geometry: WorkbenchTerminalGeometrySchema })
		.readonly(),
	z
		.strictObject({
			kind: z.literal('unavailable'),
			reason: WorkbenchTerminalUnavailableReasonSchema,
		})
		.readonly(),
	z
		.strictObject({ kind: z.literal('failed'), reason: WorkbenchTerminalFailureReasonSchema })
		.readonly(),
	z.strictObject({ kind: z.literal('outcome-unknown') }).readonly(),
]);
