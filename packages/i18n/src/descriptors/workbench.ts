import { defineMessages } from 'react-intl';

import { enUSWorkbenchMessages } from '../locales';

export const workbenchMessages = defineMessages({
	actionCategoryNavigation: {
		id: 'workbench.actions.categories.navigation',
		defaultMessage: enUSWorkbenchMessages['workbench.actions.categories.navigation'],
		description: 'Action category for navigation commands.',
	},
	actionCategoryPanels: {
		id: 'workbench.actions.categories.panels',
		defaultMessage: enUSWorkbenchMessages['workbench.actions.categories.panels'],
		description: 'Action category for Workbench panel commands.',
	},
	actionCategoryProjects: {
		id: 'workbench.actions.categories.projects',
		defaultMessage: enUSWorkbenchMessages['workbench.actions.categories.projects'],
		description: 'Action category for Project and Workspace commands.',
	},
	actionCategoryTerminal: {
		id: 'workbench.actions.categories.terminal',
		defaultMessage: enUSWorkbenchMessages['workbench.actions.categories.terminal'],
		description: 'Action category for terminal commands.',
	},
	actionCategoryView: {
		id: 'workbench.actions.categories.view',
		defaultMessage: enUSWorkbenchMessages['workbench.actions.categories.view'],
		description: 'Action category for Workbench view commands.',
	},
	actionUnavailable: {
		id: 'workbench.actions.states.unavailable',
		defaultMessage: enUSWorkbenchMessages['workbench.actions.states.unavailable'],
		description: 'Reason shown when a host does not provide an optional Workbench action.',
	},
	hideSidebar: {
		id: 'workbench.sidebar.actions.hide',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.actions.hide'],
		description: 'Action that hides the Workbench sidebar.',
	},
	newTerminal: {
		id: 'workbench.sidebar.items.newTerminal',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.newTerminal'],
		description: 'Label for the new-terminal entry in the Workbench sidebar.',
	},
	terminal: {
		id: 'workbench.sidebar.items.terminal',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.terminal'],
		description: 'Label for the standalone terminal entry in the Workbench sidebar.',
	},
	panelCancel: {
		id: 'workbench.panels.actions.cancel',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.cancel'],
		description: 'Action that cancels closing a running Workbench terminal.',
	},
	panelActivateChild: {
		id: 'workbench.panels.actions.activateChild',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.activateChild'],
		description: 'Action that activates a child terminal in a Workbench panel.',
	},
	panelChildTerminalsLabel: {
		id: 'workbench.panels.childTerminals.label',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.childTerminals.label'],
		description: 'Accessible label for the child-terminal list within a Workbench panel.',
	},
	panelClose: {
		id: 'workbench.panels.actions.close',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.close'],
		description: 'Action that closes the displayed terminal or its Workbench panel.',
	},
	panelCloseAnyway: {
		id: 'workbench.panels.actions.closeAnyway',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.closeAnyway'],
		description: 'Action that confirms closing running terminals.',
	},
	panelCloseRunningDescription: {
		id: 'workbench.panels.closeRunning.description',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.closeRunning.description'],
		description: 'Warning shown before closing one or more running terminals.',
	},
	panelCloseRunningTitle: {
		id: 'workbench.panels.closeRunning.title',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.closeRunning.title'],
		description: 'Title for the running-terminal close confirmation.',
	},
	panelMaximize: {
		id: 'workbench.panels.actions.maximize',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.maximize'],
		description: 'Action that expands one Workbench panel to the tab content area.',
	},
	panelFocus: {
		id: 'workbench.panels.actions.focus',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.focus'],
		description: 'Action that focuses a Workbench panel.',
	},
	panelMinimize: {
		id: 'workbench.panels.actions.minimize',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.minimize'],
		description: 'Action that restores a maximized Workbench panel to the split layout.',
	},
	panelMove: {
		id: 'workbench.panels.actions.move',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.move'],
		description: 'Action that moves a Workbench panel relative to another panel.',
	},
	panelMore: {
		id: 'workbench.panels.actions.more',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.more'],
		description: 'Accessible label for the Workbench panel options menu.',
	},
	panelNewChildTerminal: {
		id: 'workbench.panels.actions.newChildTerminal',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.newChildTerminal'],
		description: 'Action that creates a child terminal in the current Workbench panel.',
	},
	panelReturnToPrimary: {
		id: 'workbench.panels.actions.returnToPrimary',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.returnToPrimary'],
		description: 'Action that returns from a child terminal to the primary terminal.',
	},
	panelSplitAbove: {
		id: 'workbench.panels.actions.splitAbove',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.splitAbove'],
		description: 'Action that creates a Workbench panel above the current panel.',
	},
	panelSplit: {
		id: 'workbench.panels.actions.split',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.split'],
		description: 'Action that splits a Workbench panel in a requested direction.',
	},
	panelSplitBelow: {
		id: 'workbench.panels.actions.splitBelow',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.splitBelow'],
		description: 'Action that creates a Workbench panel below the current panel.',
	},
	panelSplitLeft: {
		id: 'workbench.panels.actions.splitLeft',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.splitLeft'],
		description: 'Action that creates a Workbench panel to the left of the current panel.',
	},
	panelSplitRight: {
		id: 'workbench.panels.actions.splitRight',
		defaultMessage: enUSWorkbenchMessages['workbench.panels.actions.splitRight'],
		description: 'Action that creates a Workbench panel to the right of the current panel.',
	},
	project: {
		id: 'workbench.sidebar.sections.project',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.sections.project'],
		description: 'Heading for the project section in the Workbench sidebar.',
	},
	projectCollapse: {
		id: 'workbench.projects.actions.collapse',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.collapse'],
		description: 'Action that collapses a Project in the Workbench sidebar.',
	},
	projectAddFolder: {
		id: 'workbench.projects.actions.addFolder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.addFolder'],
		description: 'Action that adds an existing Folder to a Project.',
	},
	projectCreate: {
		id: 'workbench.projects.actions.create',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.create'],
		description: 'Action that creates a Project from one entered Folder path.',
	},
	projectCreateDescription: {
		id: 'workbench.projects.create.description',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.create.description'],
		description: 'Explanation of how the selected Folder is used when creating a Project.',
	},
	projectCreatePath: {
		id: 'workbench.projects.create.path',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.create.path'],
		description: 'Label for the Folder path used to create a Project.',
	},
	projectCreatePathDescription: {
		id: 'workbench.projects.create.pathDescription',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.create.pathDescription'],
		description: 'Browser validation limitation shown below the Project Folder path field.',
	},
	projectCreatePathPlaceholder: {
		id: 'workbench.projects.create.pathPlaceholder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.create.pathPlaceholder'],
		description: 'Example absolute path shown in the Project Folder path field.',
	},
	projectCreateTitle: {
		id: 'workbench.projects.create.title',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.create.title'],
		description: 'Title for selecting the first Folder used to create a Project.',
	},
	projectCreateWorkspace: {
		id: 'workbench.projects.actions.createWorkspace',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.createWorkspace'],
		description: 'Action that creates a Workspace from one Project folder.',
	},
	projectExpand: {
		id: 'workbench.projects.actions.expand',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.expand'],
		description: 'Action that expands a Project in the Workbench sidebar.',
	},
	projectManage: {
		id: 'workbench.projects.actions.manage',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.manage'],
		description: 'Action that opens the Project management interface.',
	},
	projectNewFolder: {
		id: 'workbench.projects.actions.newFolder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.newFolder'],
		description: 'Action that creates a new Folder for a Project.',
	},
	projectOptions: {
		id: 'workbench.projects.actions.options',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.options'],
		description: 'Accessible label for a Project options menu.',
	},
	projectRemove: {
		id: 'workbench.projects.actions.remove',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.remove'],
		description: 'Action that removes a Project from Orchbee.',
	},
	projectRemoveFolder: {
		id: 'workbench.projects.actions.removeFolder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.removeFolder'],
		description: 'Action that removes a manually added Folder from a Project.',
	},
	projectReorder: {
		id: 'workbench.projects.actions.reorder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.reorder'],
		description: 'Action that reorders a Project in the Workbench sidebar.',
	},
	projectSetPrimaryFolder: {
		id: 'workbench.projects.actions.setPrimaryFolder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.setPrimaryFolder'],
		description: 'Action that sets the Project primary Folder.',
	},
	projectSelectFolder: {
		id: 'workbench.projects.actions.selectFolder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.selectFolder'],
		description: 'Action that selects one Folder for a new Project.',
	},
	projectToggleExpansion: {
		id: 'workbench.projects.actions.toggleExpansion',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.actions.toggleExpansion'],
		description: 'Action that toggles Project disclosure in the Workbench sidebar.',
	},
	projectRemoveDescription: {
		id: 'workbench.projects.confirmRemove.description',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.confirmRemove.description'],
		description: 'Explanation shown before removing a Project from Orchbee.',
	},
	projectRemoveTitle: {
		id: 'workbench.projects.confirmRemove.title',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.confirmRemove.title'],
		description: 'Title shown before removing a Project from Orchbee.',
	},
	projectFolders: {
		id: 'workbench.projects.groups.folders',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.groups.folders'],
		description: 'Menu group for regular Project folders.',
	},
	projectWorktrees: {
		id: 'workbench.projects.groups.worktrees',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.groups.worktrees'],
		description: 'Menu group for Git worktree folders.',
	},
	projectManageDescription: {
		id: 'workbench.projects.manage.description',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.description'],
		description: 'Accessible description for the Project management interface.',
	},
	projectManageFolders: {
		id: 'workbench.projects.manage.folders',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.folders'],
		description: 'Heading for Project folders in the Project management interface.',
	},
	projectManageName: {
		id: 'workbench.projects.manage.name',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.name'],
		description: 'Label for the Project name field.',
	},
	projectManageNamePlaceholder: {
		id: 'workbench.projects.manage.namePlaceholder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.namePlaceholder'],
		description: 'Placeholder for the Project name field.',
	},
	projectManageFolderAlreadyAdded: {
		id: 'workbench.projects.manage.folderAlreadyAdded',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.folderAlreadyAdded'],
		description: 'Explanation shown when a Folder path already belongs to the Project.',
	},
	projectManagePrimaryFolder: {
		id: 'workbench.projects.manage.primaryFolder',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.primaryFolder'],
		description: 'Label identifying a Project primary Folder.',
	},
	projectManageTitle: {
		id: 'workbench.projects.manage.title',
		defaultMessage: enUSWorkbenchMessages['workbench.projects.manage.title'],
		description: 'Title for the Project management interface.',
	},
	resourceCancel: {
		id: 'workbench.resource.actions.cancel',
		defaultMessage: enUSWorkbenchMessages['workbench.resource.actions.cancel'],
		description: 'Action that cancels a Project or Workspace edit.',
	},
	resourceAdd: {
		id: 'workbench.resource.actions.add',
		defaultMessage: enUSWorkbenchMessages['workbench.resource.actions.add'],
		description: 'Action that adds an entered Folder to a Project.',
	},
	resourceClose: {
		id: 'workbench.resource.actions.close',
		defaultMessage: enUSWorkbenchMessages['workbench.resource.actions.close'],
		description: 'Accessible label for closing a Project or Workspace dialog.',
	},
	resourceCreate: {
		id: 'workbench.resource.actions.create',
		defaultMessage: enUSWorkbenchMessages['workbench.resource.actions.create'],
		description: 'Action that creates a new Project.',
	},
	resourceSave: {
		id: 'workbench.resource.actions.save',
		defaultMessage: enUSWorkbenchMessages['workbench.resource.actions.save'],
		description: 'Action that saves a Project or Workspace edit.',
	},
	search: {
		id: 'workbench.sidebar.items.search',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.search'],
		description: 'Label for the search entry in the Workbench sidebar.',
	},
	workspaceCopySuffix: {
		id: 'workbench.workspaces.names.copySuffix',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.names.copySuffix'],
		description: 'Suffix appended to the default name of a duplicated Workspace.',
	},
	workspaceActivate: {
		id: 'workbench.workspaces.actions.activate',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.actions.activate'],
		description: 'Action that activates a Workspace terminal context.',
	},
	workspacePrimaryName: {
		id: 'workbench.workspaces.names.primary',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.names.primary'],
		description: 'Default name of the Workspace created with a Project.',
	},
	workspaceDelete: {
		id: 'workbench.workspaces.actions.delete',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.actions.delete'],
		description: 'Action that deletes a Workspace and its terminal context.',
	},
	workspaceDeleteDescription: {
		id: 'workbench.workspaces.confirmDelete.description',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.confirmDelete.description'],
		description: 'Explanation shown before deleting a Workspace.',
	},
	workspaceDeleteTitle: {
		id: 'workbench.workspaces.confirmDelete.title',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.confirmDelete.title'],
		description: 'Title shown before deleting a Workspace.',
	},
	workspaceDuplicate: {
		id: 'workbench.workspaces.actions.duplicate',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.actions.duplicate'],
		description: 'Action that duplicates a Workspace without copying running processes.',
	},
	workspaceManage: {
		id: 'workbench.workspaces.actions.manage',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.actions.manage'],
		description: 'Action that opens the Workspace management interface.',
	},
	workspaceManageDescription: {
		id: 'workbench.workspaces.manage.description',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.manage.description'],
		description: 'Accessible description for the Workspace management interface.',
	},
	workspaceManageName: {
		id: 'workbench.workspaces.manage.name',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.manage.name'],
		description: 'Label for the Workspace name field.',
	},
	workspaceManageNamePlaceholder: {
		id: 'workbench.workspaces.manage.namePlaceholder',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.manage.namePlaceholder'],
		description: 'Placeholder for the Workspace name field.',
	},
	workspaceManagePrimaryFolder: {
		id: 'workbench.workspaces.manage.primaryFolder',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.manage.primaryFolder'],
		description: 'Label for the Workspace primary Folder field.',
	},
	workspaceManageTitle: {
		id: 'workbench.workspaces.manage.title',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.manage.title'],
		description: 'Title for the Workspace management interface.',
	},
	workspaceOptions: {
		id: 'workbench.workspaces.actions.options',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.actions.options'],
		description: 'Accessible label for a Workspace options menu.',
	},
	workspaceReorder: {
		id: 'workbench.workspaces.actions.reorder',
		defaultMessage: enUSWorkbenchMessages['workbench.workspaces.actions.reorder'],
		description: 'Action that reorders a Workspace inside its owning Project.',
	},
	searchActionsNeedContext: {
		id: 'workbench.search.actions.needsContext',
		defaultMessage: enUSWorkbenchMessages['workbench.search.actions.needsContext'],
		description:
			'Explanation for a discovered action that needs a target not available in unified search.',
	},
	searchArgumentHorizontalTabs: {
		id: 'workbench.search.arguments.horizontalTabs',
		defaultMessage: enUSWorkbenchMessages['workbench.search.arguments.horizontalTabs'],
		description: 'Argument option that places the Workbench tab strip horizontally.',
	},
	searchArgumentOptions: {
		id: 'workbench.search.arguments.options',
		defaultMessage: enUSWorkbenchMessages['workbench.search.arguments.options'],
		description: 'Group heading for choices in a nested unified-search argument picker.',
	},
	searchArgumentPlaceholder: {
		id: 'workbench.search.arguments.placeholder',
		defaultMessage: enUSWorkbenchMessages['workbench.search.arguments.placeholder'],
		description: 'Placeholder for a nested unified-search argument picker.',
	},
	searchArgumentPrompt: {
		id: 'workbench.search.arguments.prompt',
		defaultMessage: enUSWorkbenchMessages['workbench.search.arguments.prompt'],
		description: 'Prompt for a nested unified-search argument picker.',
	},
	searchArgumentVerticalTabs: {
		id: 'workbench.search.arguments.verticalTabs',
		defaultMessage: enUSWorkbenchMessages['workbench.search.arguments.verticalTabs'],
		description: 'Argument option that places the Workbench tab strip vertically.',
	},
	searchDialogDescription: {
		id: 'workbench.search.dialog.description',
		defaultMessage: enUSWorkbenchMessages['workbench.search.dialog.description'],
		description: 'Accessible description for the unified search dialog.',
	},
	searchDialogTitle: {
		id: 'workbench.search.dialog.title',
		defaultMessage: enUSWorkbenchMessages['workbench.search.dialog.title'],
		description: 'Accessible title for the unified search dialog.',
	},
	searchEmptyActions: {
		id: 'workbench.search.empty.actions',
		defaultMessage: enUSWorkbenchMessages['workbench.search.empty.actions'],
		description: 'Empty state for the Actions unified-search filter.',
	},
	searchEmptyAgents: {
		id: 'workbench.search.empty.agents',
		defaultMessage: enUSWorkbenchMessages['workbench.search.empty.agents'],
		description: 'Empty state for the Agents unified-search filter.',
	},
	searchEmptyAll: {
		id: 'workbench.search.empty.all',
		defaultMessage: enUSWorkbenchMessages['workbench.search.empty.all'],
		description: 'Empty state for the All unified-search filter.',
	},
	searchEmptySettings: {
		id: 'workbench.search.empty.settings',
		defaultMessage: enUSWorkbenchMessages['workbench.search.empty.settings'],
		description: 'Empty state for the Settings unified-search filter.',
	},
	searchFilterActions: {
		id: 'workbench.search.filters.actions',
		defaultMessage: enUSWorkbenchMessages['workbench.search.filters.actions'],
		description: 'Actions filter in unified search.',
	},
	searchFilterAgents: {
		id: 'workbench.search.filters.agents',
		defaultMessage: enUSWorkbenchMessages['workbench.search.filters.agents'],
		description: 'Agents filter in unified search.',
	},
	searchFilterAll: {
		id: 'workbench.search.filters.all',
		defaultMessage: enUSWorkbenchMessages['workbench.search.filters.all'],
		description: 'All-results filter in unified search.',
	},
	searchFiltersLabel: {
		id: 'workbench.search.filters.label',
		defaultMessage: enUSWorkbenchMessages['workbench.search.filters.label'],
		description: 'Accessible name for the unified-search filter tablist.',
	},
	searchFilterSettings: {
		id: 'workbench.search.filters.settings',
		defaultMessage: enUSWorkbenchMessages['workbench.search.filters.settings'],
		description: 'Settings filter in unified search.',
	},
	searchFooterBack: {
		id: 'workbench.search.footer.back',
		defaultMessage: enUSWorkbenchMessages['workbench.search.footer.back'],
		description: 'Footer hint for returning from a nested unified-search picker.',
	},
	searchFooterClose: {
		id: 'workbench.search.footer.close',
		defaultMessage: enUSWorkbenchMessages['workbench.search.footer.close'],
		description: 'Footer hint for closing unified search.',
	},
	searchFooterRun: {
		id: 'workbench.search.footer.run',
		defaultMessage: enUSWorkbenchMessages['workbench.search.footer.run'],
		description: 'Footer hint for invoking the highlighted unified-search result.',
	},
	searchFooterSelect: {
		id: 'workbench.search.footer.select',
		defaultMessage: enUSWorkbenchMessages['workbench.search.footer.select'],
		description: 'Footer hint for moving the unified-search highlight.',
	},
	searchGroupRecommendedActions: {
		id: 'workbench.search.groups.recommendedActions',
		defaultMessage: enUSWorkbenchMessages['workbench.search.groups.recommendedActions'],
		description: 'Unified-search group containing recommended product actions.',
	},
	searchGroupSettings: {
		id: 'workbench.search.groups.settings',
		defaultMessage: enUSWorkbenchMessages['workbench.search.groups.settings'],
		description: 'Unified-search group containing settings destinations.',
	},
	searchInputActions: {
		id: 'workbench.search.input.actions',
		defaultMessage: enUSWorkbenchMessages['workbench.search.input.actions'],
		description: 'Input placeholder while the Actions unified-search filter is selected.',
	},
	searchInputAgents: {
		id: 'workbench.search.input.agents',
		defaultMessage: enUSWorkbenchMessages['workbench.search.input.agents'],
		description: 'Input placeholder while the Agents unified-search filter is selected.',
	},
	searchInputAll: {
		id: 'workbench.search.input.all',
		defaultMessage: enUSWorkbenchMessages['workbench.search.input.all'],
		description: 'Input placeholder while the All unified-search filter is selected.',
	},
	searchInputSettings: {
		id: 'workbench.search.input.settings',
		defaultMessage: enUSWorkbenchMessages['workbench.search.input.settings'],
		description: 'Input placeholder while the Settings unified-search filter is selected.',
	},
	searchProviderFailed: {
		id: 'workbench.search.providers.failed',
		defaultMessage: enUSWorkbenchMessages['workbench.search.providers.failed'],
		description: 'Status shown when one unified-search provider fails.',
	},
	searchProviderLoading: {
		id: 'workbench.search.providers.loading',
		defaultMessage: enUSWorkbenchMessages['workbench.search.providers.loading'],
		description: 'Status shown while one unified-search provider is loading.',
	},
	searchProviderRetry: {
		id: 'workbench.search.providers.retry',
		defaultMessage: enUSWorkbenchMessages['workbench.search.providers.retry'],
		description: 'Button label for retrying one failed unified-search provider.',
	},
	searchResultsCount: {
		id: 'workbench.search.results.count',
		defaultMessage: enUSWorkbenchMessages['workbench.search.results.count'],
		description: 'Live announcement for the current unified-search result count.',
	},
	searchResultsLabel: {
		id: 'workbench.search.results.label',
		defaultMessage: enUSWorkbenchMessages['workbench.search.results.label'],
		description: 'Accessible name for the unified-search results list.',
	},
	settings: {
		id: 'workbench.sidebar.items.settings',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.items.settings'],
		description: 'Label for the settings entry in the Workbench sidebar footer.',
	},
	showSidebar: {
		id: 'workbench.sidebar.actions.show',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.actions.show'],
		description: 'Action that shows the Workbench sidebar.',
	},
	toggleSidebar: {
		id: 'workbench.sidebar.actions.toggle',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.actions.toggle'],
		description: 'Action that toggles Workbench sidebar visibility.',
	},
	tabStripOrientation: {
		id: 'workbench.tabs.actions.setOrientation',
		defaultMessage: enUSWorkbenchMessages['workbench.tabs.actions.setOrientation'],
		description: 'Action that sets the Workbench tab-strip orientation.',
	},
	sidebarLabel: {
		id: 'workbench.sidebar.label',
		defaultMessage: enUSWorkbenchMessages['workbench.sidebar.label'],
		description: 'Accessible label for the Workbench navigation sidebar.',
	},
});
