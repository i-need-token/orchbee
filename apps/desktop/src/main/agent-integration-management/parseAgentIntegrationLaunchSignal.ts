import {
	DesktopAgentIntegrationIdSchema,
	type DesktopAgentIntegrationId,
} from '@orchbee/desktop-client-protocol';
import { z } from 'zod';

const AgentIntegrationLaunchSignalSchema = z.strictObject({
	integrationId: DesktopAgentIntegrationIdSchema,
	kind: z.literal('launch'),
	version: z.literal(1),
});

export interface AgentIntegrationLaunchPresence {
	readonly integrationId: DesktopAgentIntegrationId;
	readonly signatureId: string;
	readonly source: 'terminal-command';
}

const LAUNCH_SIGNATURES = {
	'claude-code': 'shell-preexec.executable.claude',
	cline: 'shell-preexec.executable.cline',
	codex: 'shell-preexec.executable.codex',
	gemini: 'shell-preexec.executable.gemini',
	'github-copilot': 'shell-preexec.executable.copilot',
	grok: 'shell-preexec.executable.grok',
	omp: 'shell-preexec.executable.omp',
	opencode: 'shell-preexec.executable.opencode',
	pi: 'shell-preexec.executable.pi',
	'qwen-code': 'shell-preexec.executable.qwen',
} as const satisfies Readonly<Record<DesktopAgentIntegrationId, string>>;

export function parseAgentIntegrationLaunchSignal(
	payload: string,
): AgentIntegrationLaunchPresence | null {
	let value: unknown;
	try {
		value = JSON.parse(payload);
	} catch {
		return null;
	}
	const parsed = AgentIntegrationLaunchSignalSchema.safeParse(value);
	if (!parsed.success) return null;
	return Object.freeze({
		integrationId: parsed.data.integrationId,
		signatureId: LAUNCH_SIGNATURES[parsed.data.integrationId],
		source: 'terminal-command' as const,
	});
}
