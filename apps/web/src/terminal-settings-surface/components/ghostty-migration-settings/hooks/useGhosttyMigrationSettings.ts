import { terminalSettingsMessages, useOrchbeeIntl } from '@orchbee/i18n';
import * as React from 'react';

import {
	analyzeGhosttyConfiguration,
	type GhosttyConfigurationAnalysis,
	type GhosttyConfigurationAnalysisFinding,
	type GhosttyConfigurationFindingReason,
	type GhosttyConfigurationMigrationTarget,
} from '../../../../ghostty-configuration-analysis';
import type {
	GhosttyMigrationDisposition,
	GhosttyMigrationFindingView,
	GhosttyMigrationReportView,
	GhosttyMigrationSettingsView,
	GhosttyMigrationTargetView,
} from '../types/ghostty-migration-settings';

export function useGhosttyMigrationSettings(): GhosttyMigrationSettingsView {
	const intl = useOrchbeeIntl();
	const [source, setSource] = React.useState('');
	const [analysis, setAnalysis] = React.useState<GhosttyConfigurationAnalysis | null>(null);
	const dispositionLabel = (disposition: GhosttyMigrationDisposition): string => {
		switch (disposition) {
			case 'supported':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationSupported);
			case 'manual':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationManual);
			case 'ignored':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationIgnored);
			case 'invalid':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationInvalid);
		}
	};

	const reasonMessage = (reason: GhosttyConfigurationFindingReason): string => {
		switch (reason) {
			case 'cell-opacity-not-supported':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonCellOpacity);
			case 'color-requires-review':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonColor);
			case 'config-file-not-loaded':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonConfigFile);
			case 'font-details-require-review':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonFontDetails);
			case 'font-family-fallback-list':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonFontFallback);
			case 'font-family-not-representable':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonFontFamily);
			case 'font-size-not-representable':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonFontSize);
			case 'invalid-value':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonInvalidValue);
			case 'keybind-requires-review':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonKeybind);
			case 'lossless-target':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonLosslessTarget);
			case 'malformed-line':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonMalformedLine);
			case 'not-in-whitelist':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonNotInWhitelist);
			case 'opacity-not-representable':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonOpacity);
			case 'reset-to-default':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonReset);
			case 'superseded':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonSuperseded);
			case 'theme-requires-review':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonTheme);
			case 'window-theme-requires-review':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonWindowTheme);
			case 'working-directory-requires-review':
				return intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonWorkingDirectory);
		}
	};
	const targetView = (target: GhosttyConfigurationMigrationTarget): GhosttyMigrationTargetView => {
		switch (target.setting) {
			case 'background-opacity':
				return {
					label: intl.formatMessage(terminalSettingsMessages.backgroundOpacityLabel),
					value: intl.formatMessage(terminalSettingsMessages.backgroundOpacityPercentValue, {
						value: target.value,
					}),
				};
			case 'custom-background':
				return {
					label: intl.formatMessage(terminalSettingsMessages.backgroundLabel),
					value: target.value,
				};
			case 'custom-foreground':
				return {
					label: intl.formatMessage(terminalSettingsMessages.foregroundLabel),
					value: target.value,
				};
			case 'font-family':
				return {
					label: intl.formatMessage(terminalSettingsMessages.fontFamilyLabel),
					value: target.value,
				};
			case 'font-size':
				return {
					label: intl.formatMessage(terminalSettingsMessages.fontSizeLabel),
					value: intl.formatNumber(target.value),
				};
			case 'macos-option-as-alt': {
				const valueMessage = {
					both: terminalSettingsMessages.macosOptionAsAltBoth,
					left: terminalSettingsMessages.macosOptionAsAltLeft,
					off: terminalSettingsMessages.macosOptionAsAltOff,
					right: terminalSettingsMessages.macosOptionAsAltRight,
				}[target.value];
				return {
					label: intl.formatMessage(terminalSettingsMessages.macosOptionAsAltLabel),
					value: intl.formatMessage(valueMessage),
				};
			}
		}
	};
	const findingView = (
		finding: GhosttyConfigurationAnalysisFinding,
	): GhosttyMigrationFindingView => {
		const customColorTarget =
			finding.disposition === 'supported' &&
			(finding.target.setting === 'custom-background' ||
				finding.target.setting === 'custom-foreground');
		return {
			disposition: finding.disposition,
			dispositionLabel: dispositionLabel(finding.disposition),
			lineLabel: intl.formatMessage(terminalSettingsMessages.ghosttyMigrationLineNumber, {
				line: finding.line,
			}),
			reason: customColorTarget
				? intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReasonCustomColorTarget)
				: reasonMessage(finding.reason),
			source: finding.source,
			target: finding.disposition === 'supported' ? targetView(finding.target) : null,
		};
	};

	let report: GhosttyMigrationReportView | null = null;
	if (analysis?.kind === 'rejected') {
		let message: string;
		switch (analysis.reason) {
			case 'line-too-long':
				message = intl.formatMessage(terminalSettingsMessages.ghosttyMigrationRejectedLineTooLong, {
					limit: analysis.limit,
					line: analysis.line,
				});
				break;
			case 'source-too-large':
				message = intl.formatMessage(
					terminalSettingsMessages.ghosttyMigrationRejectedSourceTooLarge,
					{ limit: analysis.limit },
				);
				break;
			case 'too-many-lines':
				message = intl.formatMessage(
					terminalSettingsMessages.ghosttyMigrationRejectedTooManyLines,
					{ limit: analysis.limit },
				);
				break;
		}
		report = { kind: 'rejected', message };
	} else if (analysis?.kind === 'report') {
		report = {
			findings: analysis.findings.map(findingView),
			kind: 'report',
			noAssignments: intl.formatMessage(terminalSettingsMessages.ghosttyMigrationNoAssignments),
			summaries: [
				{
					disposition: 'supported',
					label: dispositionLabel('supported'),
					value: intl.formatNumber(analysis.counts.supported),
				},
				{
					disposition: 'manual',
					label: dispositionLabel('manual'),
					value: intl.formatNumber(analysis.counts.manual),
				},
				{
					disposition: 'ignored',
					label: dispositionLabel('ignored'),
					value: intl.formatNumber(analysis.counts.ignored),
				},
				{
					disposition: 'invalid',
					label: dispositionLabel('invalid'),
					value: intl.formatNumber(analysis.counts.invalid),
				},
			],
		};
	}

	return {
		canAnalyze: source.trim().length > 0,
		canClear: source.length > 0 || report !== null,
		messages: {
			analyze: intl.formatMessage(terminalSettingsMessages.ghosttyMigrationAnalyze),
			clear: intl.formatMessage(terminalSettingsMessages.ghosttyMigrationClear),
			empty: intl.formatMessage(terminalSettingsMessages.ghosttyMigrationEmpty),
			reportTitle: intl.formatMessage(terminalSettingsMessages.ghosttyMigrationReportTitle),
			safetyNotice: intl.formatMessage(terminalSettingsMessages.ghosttyMigrationSafetyNotice),
			sourceDescription: intl.formatMessage(
				terminalSettingsMessages.ghosttyMigrationSourceDescription,
			),
			sourceLabel: intl.formatMessage(terminalSettingsMessages.ghosttyMigrationSourceLabel),
			sourcePlaceholder: intl.formatMessage(
				terminalSettingsMessages.ghosttyMigrationSourcePlaceholder,
			),
			target: intl.formatMessage(terminalSettingsMessages.ghosttyMigrationTarget),
		},
		onAnalyze: (): void => {
			if (source.trim().length === 0) return;
			setAnalysis(analyzeGhosttyConfiguration(source));
		},
		onClear: (): void => {
			setSource('');
			setAnalysis(null);
		},
		onSourceChange: (event): void => {
			setSource(event.currentTarget.value);
			setAnalysis(null);
		},
		report,
		source,
	};
}
