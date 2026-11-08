import type { DesktopWindowMessageId } from '../en-US';

export const zhCNDesktopWindowMessages = {
	'desktop.window.action.toggleFullScreen': '进入 / 退出全屏',
	'desktop.window.action.toggleMaximized': '最大化 / 还原窗口',
	'desktop.window.actionUnavailable': '当前没有可用的桌面窗口。',
	'desktop.window.category': '窗口',
	'desktop.window.dataFolderLayoutNewer':
		'{root} 上次由 Orchbee {version} 使用，该版本更改了文件夹的布局。此版本无法打开它；请使用 Orchbee {version} 或更高版本。',
	'desktop.window.dataFolderLayoutNewerUnnamed':
		'{root} 上次由更新版本的 Orchbee 使用，该版本更改了文件夹的布局。此版本无法打开它；请使用该版本或更高版本。',
	'desktop.window.dataFolderRefused': 'Orchbee 无法打开其数据文件夹',
	'desktop.window.dataFolderUnreadable':
		'Orchbee 无法读取 {record}，因此不会打开 {root}。请恢复该文件，或将文件夹移走后重新开始。',
	'desktop.window.placementNewer':
		'窗口位置由更新版本的 Orchbee 保存，因此此版本不会更改它。窗口会以默认大小打开，不会记住你上次放置的位置。',
	'desktop.window.placementNotUpdated':
		'Orchbee 无法从早期版本更新窗口位置，已保持其原样。窗口会以默认大小打开，不会记住你上次放置的位置。',
	'desktop.window.recordNoticeClose': '关闭',
	'desktop.window.maximizedUnavailableInFullScreen': '请先退出全屏，再更改窗口的最大化状态。',
} as const satisfies Readonly<Record<DesktopWindowMessageId, string>>;
