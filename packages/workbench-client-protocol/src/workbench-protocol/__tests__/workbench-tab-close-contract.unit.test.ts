import { describe, expect, it } from 'vitest';

import {
	SessionIdSchema,
	WorkbenchRequestIdSchema,
	type WorkbenchRevision,
	WorkbenchTabIdSchema,
	WorkbenchTerminalContextIdSchema,
} from '../workbench-semantic-contract';
import {
	parseWorkbenchRequest,
	parseWorkbenchResponse,
} from '../workbench-command-contract';
import {
	WorkbenchTabCloseAttemptGenerationSchema,
	WorkbenchTabCloseStatusSchema,
	type WorkbenchTabCloseResume,
	type WorkbenchTabCloseResumeResult,
} from '../workbench-tab-close-contract';

describe('Workbench Tab Close contract', () => {
	it('keeps generation distinct from revision and preserves both u64 boundaries', () => {
		for (const value of ['1', '18446744073709551615']) {
			expect(WorkbenchTabCloseAttemptGenerationSchema.parse(value)).toBe(value);
		}
		for (const value of [
			'',
			'0',
			'01',
			'+1',
			'-1',
			'1.0',
			'1e0',
			' 1',
			'18446744073709551616',
			1,
			null,
		]) {
			expect(WorkbenchTabCloseAttemptGenerationSchema.safeParse(value).success).toBe(false);
		}
		const generation = WorkbenchTabCloseAttemptGenerationSchema.parse('1');
		// @ts-expect-error -- Close generations must not acquire revision authority.
		const revision: WorkbenchRevision = generation;
		expect(revision).toBe('1');
	});

	it('preserves a typed Resume target even for a denied result', () => {
		const request: WorkbenchTabCloseResume = {
			kind: 'workbench.tab-close.resume',
			protocolVersion: 1,
			requestId: WorkbenchRequestIdSchema.parse('11111111111111111111111111111111'),
			sessionId: SessionIdSchema.parse('22222222222222222222222222222222'),
			terminalContextId: WorkbenchTerminalContextIdSchema.parse('33333333333333333333333333333333'),
			tabId: WorkbenchTabIdSchema.parse('44444444444444444444444444444444'),
			closeRequestId: WorkbenchRequestIdSchema.parse('55555555555555555555555555555555'),
			expectedAttemptGeneration:
				WorkbenchTabCloseAttemptGenerationSchema.parse('18446744073709551615'),
		};
		const response: WorkbenchTabCloseResumeResult = {
			kind: 'workbench.tab-close.resume.result',
			protocolVersion: 1,
			requestId: request.requestId,
			sessionId: request.sessionId,
			terminalContextId: request.terminalContextId,
			tabId: request.tabId,
			closeRequestId: request.closeRequestId,
			outcome: { kind: 'denied' },
		};
		expect(parseWorkbenchRequest(JSON.parse(JSON.stringify(request)))).toEqual(request);
		expect(parseWorkbenchResponse(JSON.parse(JSON.stringify(response)))).toEqual(response);
	});

	it('does not confuse an uncertain effect with a queued continuation', () => {
		expect(
			WorkbenchTabCloseStatusSchema.safeParse({ kind: 'closing', phase: 'queued' }).success,
		).toBe(true);
		expect(
			WorkbenchTabCloseStatusSchema.safeParse({
				kind: 'outcome-unknown',
				phase: 'terminate',
				reason: 'transport-lost',
			}).success,
		).toBe(true);
		expect(
			WorkbenchTabCloseStatusSchema.safeParse({
				kind: 'outcome-unknown',
				phase: 'queued',
				reason: 'transport-lost',
			}).success,
		).toBe(false);
		expect(
			WorkbenchTabCloseStatusSchema.safeParse({ kind: 'closing', phase: 'terminate' }).success,
		).toBe(false);
	});
});
