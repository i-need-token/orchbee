import { Button, Textarea } from '@orchbee/ui/components';
import * as React from 'react';

import { useGhosttyMigrationSettings } from './hooks/useGhosttyMigrationSettings';
import styles from './GhosttyMigrationSettings.module.css';

export function GhosttyMigrationSettings(): React.ReactNode {
	const view = useGhosttyMigrationSettings();
	const report = view.report;
	let reportContent: React.ReactNode;
	if (report === null) {
		reportContent = <p className={styles['empty']}>{view.messages.empty}</p>;
	} else if (report.kind === 'rejected') {
		reportContent = (
			<div className={styles['rejection']} role='alert'>
				{report.message}
			</div>
		);
	} else {
		reportContent = (
			<section aria-labelledby='ghostty-migration-report-title' className={styles['report']}>
				<h2 className={styles['reportTitle']} id='ghostty-migration-report-title'>
					{view.messages.reportTitle}
				</h2>
				<dl className={styles['summary']} role='status'>
					{report.summaries.map((summary) => (
						<div
							className={styles['summaryItem']}
							data-disposition={summary.disposition}
							key={summary.disposition}
						>
							<dt>{summary.label}</dt>
							<dd>{summary.value}</dd>
						</div>
					))}
				</dl>
				{report.findings.length === 0 ? (
					<p className={styles['empty']}>{report.noAssignments}</p>
				) : (
					<ol className={styles['findings']}>
						{report.findings.map((finding) => (
							<li
								className={styles['finding']}
								data-disposition={finding.disposition}
								key={finding.lineLabel}
							>
								<div className={styles['findingHeader']}>
									<span className={styles['line']}>{finding.lineLabel}</span>
									<span className={styles['disposition']}>{finding.dispositionLabel}</span>
								</div>
								<code className={styles['findingSource']}>{finding.source}</code>
								<p className={styles['reason']}>{finding.reason}</p>
								{finding.target === null ? null : (
									<div className={styles['target']}>
										<span className={styles['targetLabel']}>{view.messages.target}</span>
										<span>{finding.target.label}</span>
										<code className={styles['targetValue']}>{finding.target.value}</code>
									</div>
								)}
							</li>
						))}
					</ol>
				)}
			</section>
		);
	}

	return (
		<section className={styles['root']} data-slot='ghostty-migration-settings'>
			<p className={styles['notice']} id='ghostty-migration-safety'>
				{view.messages.safetyNotice}
			</p>
			<div className={styles['field']} id='ghostty-migration.source'>
				<label className={styles['label']} htmlFor='ghostty-migration-source'>
					{view.messages.sourceLabel}
				</label>
				<p className={styles['description']} id='ghostty-migration-source-description'>
					{view.messages.sourceDescription}
				</p>
				<Textarea
					autoCapitalize='off'
					autoComplete='off'
					className={styles['source']}
					id='ghostty-migration-source'
					placeholder={view.messages.sourcePlaceholder}
					rows={10}
					spellCheck={false}
					value={view.source}
					aria-describedby='ghostty-migration-source-description ghostty-migration-safety'
					onChange={view.onSourceChange}
				/>
			</div>
			<div className={styles['actions']}>
				<Button disabled={!view.canAnalyze} onClick={view.onAnalyze}>
					{view.messages.analyze}
				</Button>
				<Button disabled={!view.canClear} variant='ghost' onClick={view.onClear}>
					{view.messages.clear}
				</Button>
			</div>

			{reportContent}
		</section>
	);
}
