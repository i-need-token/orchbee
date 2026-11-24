/**
 * Whether Desktop offers remote machines. Until the window's machines ship, Main starts no machines
 * process and closes its end of each document's machines port at once, so Desktop holds no hub
 * attachment and no SSH connection for anyone.
 */
export const DESKTOP_REMOTE_MACHINES = false;
