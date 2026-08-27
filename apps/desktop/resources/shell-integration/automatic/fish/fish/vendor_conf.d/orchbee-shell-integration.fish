# Orchbee automatic Fish loader. Discovered once through XDG_DATA_DIRS.

set -l __orchbee_automatic_root "$ORCHBEE_SHELL_INTEGRATION_DIR"
if set -q ORCHBEE_SHELL_INTEGRATION_ORIGINAL_XDG_DATA_DIRS
	set --global --export XDG_DATA_DIRS "$ORCHBEE_SHELL_INTEGRATION_ORIGINAL_XDG_DATA_DIRS"
	set --erase ORCHBEE_SHELL_INTEGRATION_ORIGINAL_XDG_DATA_DIRS
else
	set --erase --global XDG_DATA_DIRS
end

if status is-interactive; and test -r "$__orchbee_automatic_root/orchbee.fish"
	source "$__orchbee_automatic_root/orchbee.fish"
end

set --erase __orchbee_automatic_root
