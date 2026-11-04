interface WorkbenchAgentNotificationRequest {
	readonly body: string;
	readonly onActivate: () => void;
	readonly tag: string;
	readonly title: string;
}

/** A terminal Agent state the person has not seen, after the Workbench's delay. */
export interface WorkbenchAgentNotification {
	/** One exact Agent state; a Client notifies about it at most once. */
	readonly key: string;
	readonly notification: WorkbenchAgentNotificationRequest;
}

/** Owns notification preferences, permission and delivery. */
export interface WorkbenchAgentNotificationObserver {
	observeAgentNotification(notification: WorkbenchAgentNotification): unknown;
}
