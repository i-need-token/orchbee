import { sourceControlMessages, workbenchMessages } from '@orchbee/i18n';

/** Where Source Control's Git commands sit, as VS Code groups its Git extension's commands. */
export const SCM_GIT_CATEGORY = { id: 'git', title: sourceControlMessages.actionCategory };

/** Where the view's own commands sit, such as View as Tree and Collapse All. */
export const SCM_VIEW_CATEGORY = { id: 'view', title: workbenchMessages.actionCategoryView };
