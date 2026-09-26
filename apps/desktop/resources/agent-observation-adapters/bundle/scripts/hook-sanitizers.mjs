const IDENTITY_PATTERN = /^[A-Za-z0-9._:-]{1,128}$/u;

export function sanitizeAgentHookInput(adapterId, value) {
	if (!isRecord(value)) return null;
	switch (adapterId) {
		case 'antigravity-cli': {
			if (value.hook_event_name !== undefined && value.hook_event_name !== 'PreInvocation')
				return null;
			if (value.hookEventName !== undefined && value.hookEventName !== 'PreInvocation') return null;
			const sessionKey = identity(value.conversationId);
			return sessionKey === null ? null : observation(sessionKey, 'session-identified');
		}
		case 'qodercli': {
			const sessionKey = identity(value.session_id);
			return value.hook_event_name === 'SessionStart' && sessionKey !== null
				? observation(sessionKey, 'session-identified')
				: null;
		}
		case 'cursor':
			return sanitizeCursorHook(value);
		case 'claude-code':
			return sanitizeClaudeCodeHook(value);
		case 'codex':
			return sanitizeCodexHook(value);
		case 'grok':
			return sanitizeGrokHook(value);
		case 'gemini':
			return sanitizeGeminiHook(value);
		case 'github-copilot':
			return sanitizeGitHubCopilotHook(value);
		case 'qwen-code':
			return sanitizeQwenCodeHook(value);
		default:
			return null;
	}
}

export function sanitizeCursorHook(value) {
	if (!isRecord(value)) return null;
	if (value.hook_event_name !== undefined && value.hook_event_name !== 'sessionStart') return null;
	if (value.hookEventName !== undefined && value.hookEventName !== 'sessionStart') return null;
	const supplied = [
		value.session_id,
		value.sessionId,
		value.conversation_id,
		value.conversationId,
	].filter((entry) => entry !== undefined);
	if (supplied.length === 0) return null;
	const sessionKey = identity(supplied[0]);
	if (sessionKey === null || supplied.some((entry) => identity(entry) !== sessionKey)) return null;
	return observation(sessionKey, 'session-identified');
}

export function sanitizeClaudeCodeHook(value) {
	if (!isRecord(value)) return null;
	const sessionKey = identity(value.session_id);
	const eventName = hookEventName(value.hook_event_name);
	const turnKey = nullableIdentity(value.turn_id);
	if (sessionKey === null || eventName === null || turnKey === undefined) return null;
	const toolName = identity(value.tool_name);
	const toolUseId = identity(value.tool_use_id);
	const agentKey = firstIdentity(value.agent_id, value.subagent_id);

	switch (eventName) {
		case 'sessionstart':
			return observation(sessionKey, 'session-started', { turnKey });
		case 'userpromptsubmit':
			return observation(sessionKey, 'turn-started', { activity: 'thinking', turnKey });
		case 'pretooluse':
			if (toolName === 'AskUserQuestion') {
				return observation(sessionKey, 'question-requested', {
					operationKey: toolUseId,
					turnKey,
				});
			}
			if (toolName === 'ExitPlanMode') {
				return observation(sessionKey, 'approval-requested', {
					operationKey: toolUseId,
					turnKey,
				});
			}
			return observation(sessionKey, 'tool-started', {
				activity: claudeCodeToolActivity(toolName),
				operationKey: toolUseId,
				turnKey,
			});
		case 'permissionrequest':
			return observation(sessionKey, 'approval-requested', {
				operationKey: toolUseId,
				turnKey,
			});
		case 'posttooluse':
			return observation(sessionKey, 'tool-finished', {
				operationKey: toolUseId,
				turnKey,
			});
		case 'posttoolusefailure':
			return observation(sessionKey, 'tool-failed', {
				operationKey: toolUseId,
				turnKey,
			});
		case 'precompact':
			return observation(sessionKey, 'compaction-started', { turnKey });
		case 'postcompact':
			return observation(sessionKey, 'compaction-finished', { turnKey });
		case 'subagentstart':
			return observation(sessionKey, 'subagent-started', { agentKey, turnKey });
		case 'subagentstop':
			return observation(sessionKey, 'subagent-stopped', { agentKey, turnKey });
		case 'stop':
			return observation(sessionKey, 'turn-stopped', { turnKey });
		case 'stopfailure':
			return observation(sessionKey, 'turn-failed', { turnKey });
		case 'sessionend':
			return observation(sessionKey, 'session-ended', { turnKey });
		default:
			return null;
	}
}

export function sanitizeCodexHook(value) {
	if (!isRecord(value)) return null;
	const sessionKey = identity(value.session_id);
	const eventName = hookEventName(value.hook_event_name);
	const turnKey = nullableIdentity(value.turn_id);
	if (sessionKey === null || eventName === null || turnKey === undefined) return null;
	const toolName = identity(value.tool_name);
	const operationKey = turnKey;
	const agentKey = firstIdentity(value.agent_id, value.subagent_id);

	switch (eventName) {
		case 'sessionstart':
			return value.source === 'compact'
				? null
				: observation(sessionKey, 'session-started', { turnKey });
		case 'userpromptsubmit':
			return observation(sessionKey, 'turn-started', { activity: 'thinking', turnKey });
		case 'pretooluse':
			if (toolName === 'request_user_input' || toolName === 'AskUserQuestion') {
				return observation(sessionKey, 'question-requested', { operationKey, turnKey });
			}
			return observation(sessionKey, 'tool-started', {
				activity: codexToolActivity(toolName),
				operationKey,
				turnKey,
			});
		case 'permissionrequest':
			return observation(sessionKey, 'approval-requested', { operationKey, turnKey });
		case 'posttooluse':
			return observation(sessionKey, 'tool-finished', { operationKey, turnKey });
		case 'precompact':
			return observation(sessionKey, 'compaction-started', { turnKey });
		case 'postcompact':
			return observation(sessionKey, 'compaction-finished', { turnKey });
		case 'subagentstart':
			return observation(sessionKey, 'subagent-started', { agentKey, turnKey });
		case 'subagentstop':
			return observation(sessionKey, 'subagent-stopped', { agentKey, turnKey });
		case 'stop':
			return observation(sessionKey, 'turn-stopped', { turnKey });
		case 'sessionend':
			return observation(sessionKey, 'session-ended', { turnKey });
		default:
			return null;
	}
}

export function sanitizeGrokHook(value) {
	if (!isRecord(value)) return null;
	const sessionKey = identity(value.sessionId);
	const eventName = hookEventName(value.hookEventName);
	const turnKey = nullableIdentity(value.promptId);
	if (sessionKey === null || eventName === null || turnKey === undefined) return null;
	if (
		identity(value.subagentType) !== null &&
		eventName !== 'subagentstart' &&
		eventName !== 'subagentstop'
	) {
		return null;
	}
	const toolName = identity(value.toolName);
	const toolUseId = identity(value.toolUseId);
	const operationKey = toolUseId ?? turnKey;
	const agentKey = firstIdentity(value.agentId, value.subagentId, value.promptId);

	switch (eventName) {
		case 'sessionstart':
			return observation(sessionKey, 'session-started', { turnKey });
		case 'userpromptsubmit':
			return observation(sessionKey, 'turn-started', { activity: 'thinking', turnKey });
		case 'pretooluse':
			return observation(sessionKey, 'tool-started', {
				activity: grokToolActivity(toolName),
				operationKey,
				turnKey,
			});
		case 'posttooluse':
			return observation(sessionKey, 'tool-finished', { operationKey, turnKey });
		case 'posttoolfailure':
		case 'posttoolusefailure':
			return observation(sessionKey, 'tool-failed', { operationKey, turnKey });
		case 'permissiondenied':
			return observation(sessionKey, 'attention-resolved', { operationKey, turnKey });
		case 'notification':
			switch (value.notificationType) {
				case 'permission_prompt':
					return observation(sessionKey, 'approval-requested', { operationKey, turnKey });
				case 'idle_prompt':
					return observation(sessionKey, 'session-idle', { turnKey });
				case 'task_complete':
					return observation(sessionKey, 'turn-stopped', { turnKey });
				default:
					return null;
			}
		case 'precompact':
			return observation(sessionKey, 'compaction-started', { turnKey });
		case 'postcompact':
			return observation(sessionKey, 'compaction-finished', { turnKey });
		case 'subagentstart':
			return observation(sessionKey, 'subagent-started', { agentKey, turnKey });
		case 'subagentstop':
			return observation(sessionKey, 'subagent-stopped', { agentKey, turnKey });
		case 'stop':
			return value.reason === 'end_turn'
				? observation(sessionKey, 'turn-stopped', { turnKey })
				: null;
		case 'stopfailure':
			return observation(sessionKey, 'turn-failed', { turnKey });
		case 'stopcancelled':
		case 'stopcanceled':
			return observation(
				sessionKey,
				value.reason === 'user_interrupt' && value.cancelledBy === 'user'
					? 'turn-interrupted'
					: 'turn-canceled',
				{ turnKey },
			);
		case 'sessionend':
			return observation(sessionKey, 'session-ended', { turnKey });
		default:
			return null;
	}
}

export function sanitizeGeminiHook(value) {
	if (!isRecord(value)) return null;
	const sessionKey = identity(value.session_id);
	const eventName = hookEventName(value.hook_event_name);
	if (sessionKey === null || eventName === null) return null;
	const toolName = identity(value.tool_name);
	const operationKey = sessionKey;

	switch (eventName) {
		case 'sessionstart':
			return observation(sessionKey, 'session-started');
		case 'beforeagent':
			return observation(sessionKey, 'turn-started', { activity: 'thinking' });
		case 'beforetool':
			if (toolName === 'ask_user') {
				return observation(sessionKey, 'question-requested', { operationKey });
			}
			return observation(sessionKey, 'tool-started', {
				activity: geminiToolActivity(toolName),
				operationKey,
			});
		case 'aftertool':
			return observation(sessionKey, 'tool-finished', { operationKey });
		case 'notification':
			return value.notification_type === 'ToolPermission'
				? observation(sessionKey, 'approval-requested', { operationKey })
				: null;
		case 'precompress':
			return observation(sessionKey, 'compaction-started');
		case 'afteragent':
			return observation(sessionKey, 'turn-stopped');
		case 'sessionend':
			return observation(sessionKey, 'session-ended');
		default:
			return null;
	}
}

export function sanitizeGitHubCopilotHook(value) {
	if (!isRecord(value)) return null;
	const sessionKey = identity(value.session_id);
	const eventName = hookEventName(value.hook_event_name);
	if (sessionKey === null || eventName === null) return null;
	const operationKey = sessionKey;

	switch (eventName) {
		case 'sessionstart':
			return observation(sessionKey, 'session-started');
		case 'userpromptsubmit':
			return observation(sessionKey, 'turn-started', { activity: 'thinking' });
		case 'posttooluse':
			return observation(sessionKey, 'tool-finished', { operationKey });
		case 'posttoolusefailure':
			return observation(sessionKey, 'tool-failed', { operationKey });
		case 'notification':
			switch (value.notification_type) {
				case 'permission_prompt':
					return observation(sessionKey, 'approval-requested', { operationKey });
				case 'elicitation_dialog':
					return observation(sessionKey, 'question-requested', { operationKey });
				default:
					return null;
			}
		case 'precompact':
			return observation(sessionKey, 'compaction-started');
		case 'stop':
			return observation(sessionKey, 'turn-stopped');
		case 'erroroccurred':
			return value.recoverable === false ? observation(sessionKey, 'turn-failed') : null;
		case 'sessionend':
			return observation(sessionKey, 'session-ended');
		default:
			return null;
	}
}

export function sanitizeQwenCodeHook(value) {
	if (!isRecord(value)) return null;
	const sessionKey = identity(value.session_id);
	const eventName = hookEventName(value.hook_event_name);
	if (sessionKey === null || eventName === null) return null;
	const toolName = identity(value.tool_name);
	const operationKey = sessionKey;
	const agentKey = identity(value.agent_id);

	switch (eventName) {
		case 'sessionstart':
			return value.source === 'compact' ? null : observation(sessionKey, 'session-started');
		case 'userpromptsubmit':
			return typeof value.submitted_prompt === 'string'
				? observation(sessionKey, 'turn-started', { activity: 'thinking' })
				: null;
		case 'pretooluse':
			if (toolName === 'ask_user_question') {
				return observation(sessionKey, 'question-requested', { operationKey });
			}
			return observation(sessionKey, 'tool-started', {
				activity: qwenCodeToolActivity(toolName),
				operationKey,
			});
		case 'posttooluse':
			return observation(sessionKey, 'tool-finished', { operationKey });
		case 'posttoolusefailure':
			return observation(sessionKey, 'tool-failed', { operationKey });
		case 'permissionrequest':
			return observation(sessionKey, 'approval-requested', { operationKey: sessionKey });
		case 'permissiondenied':
			return observation(sessionKey, 'attention-resolved', { operationKey: sessionKey });
		case 'notification':
			switch (value.notification_type) {
				case 'permission_prompt':
					return observation(sessionKey, 'approval-requested', {
						operationKey: sessionKey,
					});
				case 'idle_prompt':
					return observation(sessionKey, 'session-idle');
				default:
					return null;
			}
		case 'precompact':
			return observation(sessionKey, 'compaction-started');
		case 'postcompact':
			return observation(sessionKey, 'compaction-finished');
		case 'subagentstart':
			return observation(sessionKey, 'subagent-started', { agentKey });
		case 'subagentstop':
			return observation(sessionKey, 'subagent-stopped', { agentKey });
		case 'stop':
			return observation(sessionKey, 'turn-stopped');
		case 'stopfailure':
			return observation(sessionKey, 'turn-failed');
		case 'sessionend':
			return observation(sessionKey, 'session-ended');
		default:
			return null;
	}
}

function observation(sessionKey, event, fields = {}) {
	return {
		activity: fields.activity ?? null,
		agentKey: fields.agentKey ?? null,
		event,
		operationKey: fields.operationKey ?? null,
		sessionKey,
		turnKey: fields.turnKey ?? null,
		version: 1,
	};
}

function claudeCodeToolActivity(toolName) {
	if (toolName === 'Bash') return 'command';
	if (toolName === 'Edit' || toolName === 'Write' || toolName === 'NotebookEdit') return 'editing';
	if (toolName === 'Task' || toolName === 'Agent') return 'subagent';
	return 'tool';
}

function codexToolActivity(toolName) {
	if (toolName === 'exec_command' || toolName === 'write_stdin') return 'command';
	if (toolName === 'apply_patch') return 'editing';
	if (toolName === 'spawn_agent' || toolName === 'send_input') return 'subagent';
	return 'tool';
}

function grokToolActivity(toolName) {
	if (toolName === 'run_terminal_command') return 'command';
	if (toolName === 'search_replace' || toolName === 'write_file') return 'editing';
	if (toolName === 'spawn_subagent') return 'subagent';
	return 'tool';
}

function geminiToolActivity(toolName) {
	if (toolName === 'run_shell_command') return 'command';
	if (toolName === 'replace' || toolName === 'write_file') return 'editing';
	if (toolName === 'delegate_to_agent') return 'subagent';
	return 'tool';
}

function qwenCodeToolActivity(toolName) {
	if (toolName === 'run_shell_command') return 'command';
	if (toolName === 'edit' || toolName === 'write_file') return 'editing';
	if (toolName === 'task') return 'subagent';
	return 'tool';
}

function hookEventName(value) {
	if (typeof value !== 'string' || value.length === 0 || value.length > 64) return null;
	return value.replaceAll(/[^A-Za-z]/gu, '').toLowerCase();
}

function firstIdentity(...values) {
	for (const value of values) {
		const result = identity(value);
		if (result !== null) return result;
	}
	return null;
}

function nullableIdentity(value) {
	if (value === undefined || value === null) return null;
	const result = identity(value);
	return result === null ? undefined : result;
}

function identity(value) {
	return typeof value === 'string' && IDENTITY_PATTERN.test(value) ? value : null;
}

function isRecord(value) {
	return typeof value === 'object' && value !== null && !Array.isArray(value);
}
