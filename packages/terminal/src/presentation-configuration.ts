import { parseFontMetricModifier } from './font-metrics';
import type { FontMetricModifiers, FontWeight, PaddingBalance, RendererOptions } from './types';

/** Complete client-owned presentation state changed by one runtime transaction. */
export interface TerminalPresentationConfiguration {
	readonly allowTransparency: boolean;
	readonly fontFamily: string;
	readonly fontLigatures: boolean;
	readonly fontMetricModifiers: FontMetricModifiers;
	readonly fontSize: number;
	readonly fontWeight: FontWeight;
	readonly fontWeightBold: FontWeight;
	readonly paddingBalance: PaddingBalance;
}

export type TerminalPresentationConfigurationValidationResult =
	| {
			readonly configuration: TerminalPresentationConfiguration;
			readonly kind: 'valid';
	  }
	| {
			readonly field: string;
			readonly kind: 'invalid';
	  };

export type TerminalPresentationConfigurationUpdateResult =
	| {
			readonly configuration: TerminalPresentationConfiguration;
			readonly kind: 'completed';
	  }
	| {
			readonly kind: 'cancelled';
			readonly reason: 'disposed' | 'superseded';
	  }
	| {
			readonly code:
				| 'font-preparation-failed'
				| 'renderer-preparation-failed'
				| 'terminal-not-open';
			readonly kind: 'failed';
	  }
	| {
			readonly code: 'invalid-configuration';
			readonly field: string;
			readonly kind: 'failed';
	  };

const CONFIGURATION_FIELDS = [
	'allowTransparency',
	'fontFamily',
	'fontLigatures',
	'fontMetricModifiers',
	'fontSize',
	'fontWeight',
	'fontWeightBold',
	'paddingBalance',
] as const;

const METRIC_FIELDS = [
	'boxThickness',
	'cellHeight',
	'cellWidth',
	'cursorHeight',
	'cursorThickness',
	'fontBaseline',
	'iconHeight',
	'overlinePosition',
	'overlineThickness',
	'strikethroughPosition',
	'strikethroughThickness',
	'underlinePosition',
	'underlineThickness',
] as const;

export const DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION: TerminalPresentationConfiguration =
	Object.freeze({
		allowTransparency: false,
		fontFamily: 'monospace',
		fontLigatures: true,
		fontMetricModifiers: Object.freeze({}),
		fontSize: 15,
		fontWeight: 'normal',
		fontWeightBold: 'bold',
		paddingBalance: false,
	});

/** Validate one complete transaction and isolate it from caller mutation. */
export function normalizeTerminalPresentationConfiguration(
	input: unknown,
): TerminalPresentationConfigurationValidationResult {
	try {
		return normalizeTerminalPresentationConfigurationUnchecked(input);
	} catch {
		return invalid('configuration');
	}
}

function normalizeTerminalPresentationConfigurationUnchecked(
	input: unknown,
): TerminalPresentationConfigurationValidationResult {
	if (!isRecord(input) || !hasExactFields(input, CONFIGURATION_FIELDS)) {
		return invalid('configuration');
	}

	const allowTransparency = Reflect.get(input, 'allowTransparency');
	if (typeof allowTransparency !== 'boolean') return invalid('allowTransparency');
	const fontFamily = Reflect.get(input, 'fontFamily');
	if (typeof fontFamily !== 'string' || fontFamily.trim().length === 0) {
		return invalid('fontFamily');
	}
	const fontLigatures = Reflect.get(input, 'fontLigatures');
	if (typeof fontLigatures !== 'boolean') return invalid('fontLigatures');
	const fontMetricModifiers = normalizeFontMetricModifiers(
		Reflect.get(input, 'fontMetricModifiers'),
	);
	if (fontMetricModifiers === undefined) return invalid('fontMetricModifiers');
	const fontSize = Reflect.get(input, 'fontSize');
	if (!isPositiveFiniteNumber(fontSize)) return invalid('fontSize');
	const fontWeight = Reflect.get(input, 'fontWeight');
	if (!isFontWeight(fontWeight)) return invalid('fontWeight');
	const fontWeightBold = Reflect.get(input, 'fontWeightBold');
	if (!isFontWeight(fontWeightBold)) return invalid('fontWeightBold');
	const paddingBalance = Reflect.get(input, 'paddingBalance');
	if (!isPaddingBalance(paddingBalance)) return invalid('paddingBalance');

	return {
		configuration: Object.freeze({
			allowTransparency,
			fontFamily: fontFamily.trim(),
			fontLigatures,
			fontMetricModifiers: Object.freeze(fontMetricModifiers),
			fontSize,
			fontWeight,
			fontWeightBold,
			paddingBalance,
		}),
		kind: 'valid',
	};
}

export function presentationConfigurationFromRendererOptions(
	options: RendererOptions | undefined,
): TerminalPresentationConfiguration {
	const defaults = DEFAULT_TERMINAL_PRESENTATION_CONFIGURATION;
	const result = normalizeTerminalPresentationConfiguration({
		allowTransparency: options?.allowTransparency ?? defaults.allowTransparency,
		fontFamily: options?.fontFamily ?? defaults.fontFamily,
		fontLigatures: options?.fontLigatures ?? defaults.fontLigatures,
		fontMetricModifiers: options?.fontMetricModifiers ?? defaults.fontMetricModifiers,
		fontSize: options?.fontSize ?? defaults.fontSize,
		fontWeight: options?.fontWeight ?? defaults.fontWeight,
		fontWeightBold: options?.fontWeightBold ?? defaults.fontWeightBold,
		paddingBalance: options?.paddingBalance ?? defaults.paddingBalance,
	});
	if (result.kind === 'invalid') {
		throw new TypeError(`Invalid terminal presentation configuration field: ${result.field}`);
	}
	return result.configuration;
}

function normalizeFontMetricModifiers(input: unknown): FontMetricModifiers | undefined {
	if (!isRecord(input)) return undefined;
	const keys = Reflect.ownKeys(input);
	if (
		keys.some((key) => typeof key !== 'string' || !METRIC_FIELDS.some((field) => field === key))
	) {
		return undefined;
	}
	const modifiers: {
		-readonly [Key in keyof FontMetricModifiers]: FontMetricModifiers[Key];
	} = {};
	for (const field of METRIC_FIELDS) {
		if (!Object.hasOwn(input, field)) continue;
		const value = Reflect.get(input, field);
		if (value === undefined) continue;
		if (typeof value !== 'number' && typeof value !== 'string') return undefined;
		try {
			parseFontMetricModifier(value);
		} catch {
			return undefined;
		}
		modifiers[field] = value;
	}
	return modifiers;
}

function hasExactFields(input: object, fields: readonly string[]): boolean {
	const keys = Reflect.ownKeys(input);
	return (
		keys.length === fields.length &&
		keys.every((key) => typeof key === 'string' && fields.includes(key))
	);
}

function invalid(field: string): TerminalPresentationConfigurationValidationResult {
	return { field, kind: 'invalid' };
}

function isFontWeight(input: unknown): input is FontWeight {
	return (
		input === 'normal' ||
		input === 'bold' ||
		(typeof input === 'number' && Number.isInteger(input) && input >= 1 && input <= 1_000)
	);
}

function isPaddingBalance(input: unknown): input is PaddingBalance {
	return input === false || input === true || input === 'equal';
}

function isPositiveFiniteNumber(input: unknown): input is number {
	return typeof input === 'number' && Number.isFinite(input) && input > 0;
}

function isRecord(input: unknown): input is object {
	return typeof input === 'object' && input !== null && !Array.isArray(input);
}
