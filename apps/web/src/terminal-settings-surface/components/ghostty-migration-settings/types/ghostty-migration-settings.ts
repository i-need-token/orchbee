import type * as React from 'react';

export type GhosttyMigrationDisposition = 'ignored' | 'invalid' | 'manual' | 'supported';

interface GhosttyMigrationSummaryView {
	readonly disposition: GhosttyMigrationDisposition;
	readonly label: string;
	readonly value: string;
}

export interface GhosttyMigrationTargetView {
	readonly label: string;
	readonly value: string;
}

export interface GhosttyMigrationFindingView {
	readonly disposition: GhosttyMigrationDisposition;
	readonly dispositionLabel: string;
	readonly lineLabel: string;
	readonly reason: string;
	readonly source: string;
	readonly target: GhosttyMigrationTargetView | null;
}

export type GhosttyMigrationReportView =
	| {
			readonly findings: readonly GhosttyMigrationFindingView[];
			readonly kind: 'report';
			readonly noAssignments: string;
			readonly summaries: readonly GhosttyMigrationSummaryView[];
	  }
	| { readonly kind: 'rejected'; readonly message: string };

interface GhosttyMigrationSettingsMessages {
	readonly analyze: string;
	readonly clear: string;
	readonly empty: string;
	readonly reportTitle: string;
	readonly safetyNotice: string;
	readonly sourceDescription: string;
	readonly sourceLabel: string;
	readonly sourcePlaceholder: string;
	readonly target: string;
}

export interface GhosttyMigrationSettingsView {
	readonly canAnalyze: boolean;
	readonly canClear: boolean;
	readonly messages: GhosttyMigrationSettingsMessages;
	readonly onAnalyze: React.MouseEventHandler<HTMLButtonElement>;
	readonly onClear: React.MouseEventHandler<HTMLButtonElement>;
	readonly onSourceChange: React.ChangeEventHandler<HTMLTextAreaElement>;
	readonly report: GhosttyMigrationReportView | null;
	readonly source: string;
}
