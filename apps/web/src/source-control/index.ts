export { SourceControl } from './SourceControl';
export { SCM_STATUS_MESSAGES } from './constants/scm-status-messages';
export type { ScmStatusColor } from './model/scm-resource';
export { scmFileDecorations, type ScmFileDecoration } from './model/scmFileDecorations';
export type { ScmRepositoryState } from './model/ScmWorkspace';
export { scmWorkspaceOf } from './model/scmWorkspaceOf';
export { useSourceControlCount } from './hooks/useSourceControlCount';
export type { SourceControlFocus } from './types/source-control-focus';
export type {
	SourceControlNotifications,
	SourceControlQuickInput,
} from './types/source-control-ports';
