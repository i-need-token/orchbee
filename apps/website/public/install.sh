#!/bin/sh
# Installs the Orchbee command line on macOS or Linux from the release the stable channel names,
# and on macOS with --desktop the window application too:
#
#   curl -fsSL https://orchbee.com/install.sh | sh
#   curl -fsSL https://orchbee.com/install.sh | sh -s -- [--desktop] [--no-path] [--version <version>]
#
# 1. Takes the platform token from uname: macos-aarch64, macos-x86_64, linux-x86_64 or
#    linux-aarch64. A shell under Rosetta on Apple silicon still installs macos-aarch64.
#    On a Mac older than macOS 13 it stops here, before anything is downloaded.
# 2. Reads that platform's feed document, https://orchbee.com/update/stable/<platform>/feed.json,
#    or with --version the feed asset of that GitHub release. A platform without a feed is told so
#    in one sentence, and nothing is downloaded or installed.
# 3. Downloads the .tar.gz the feed names, checks its size and SHA-256 against the feed, unpacks
#    it into a temporary folder and runs its `orchbee install`, which verifies the signed
#    release.json, activates the version under ~/.local/share/orchbee, links ~/.local/bin/orchbee,
#    and prints the line that puts ~/.local/bin on PATH when the shell lacks it. --no-path is
#    passed on.
# 4. With --desktop on macOS, it first downloads the window archive the feed names in
#    extensions.desktop and checks its size and SHA-256 against the feed; after the install it
#    unpacks it, has the application's bundled Client add that window version to the store with
#    --installer-window-v1, so the application can update itself, then copies the application
#    into /Applications when that folder is writable and otherwise into ~/Applications, replacing an
#    older copy there, and verifies its signature. A release whose window updates are not switched
#    on yet does not know that word; its application is copied alone. A download through curl
#    carries no quarantine attribute, so the application opens without the Gatekeeper prompt a
#    browser download meets.
# 5. Removes the temporary folder on every exit, interrupted or not.
#
# The feed signature is not checked here: `orchbee install` verifies the release directory's
# signed release.json against the keys compiled into it, so a tampered archive installs nothing.
# Messages follow LC_ALL, LC_MESSAGES or LANG: zh-CN for any zh value, otherwise en-US.
#
# Two variables exist for tests and rehearsals only:
#   ORCHBEE_INSTALL_SH_FEED          the feed document address in place of orchbee.com's: HTTPS,
#                                    http://127.0.0.1:<port>/…, or file:///…; only with it may the
#                                    feed name its archive by a path relative to the feed
#   ORCHBEE_INSTALL_SH_APPLICATIONS  the absolute folder --desktop copies the application into
#
# Exit codes: 0 installed; 1 nothing or not everything was installed, with the reason; 2 malformed
# arguments; otherwise the exit code of `orchbee install`, or 128 + the signal on an interruption.
#
# The whole script is a set of functions called on its last line, so a download cut short runs
# nothing.

ob_feed_root='https://orchbee.com/update/stable'
ob_release_root='https://github.com/i-need-token/orchbee/releases/download'
ob_feed_limit=65536
ob_archive_limit=1073741824

ob_zh=0
ob_tmp=''
ob_staged=''
ob_previous=''
ob_target=''

ob_choose_locale() {
	for ob_name in LC_ALL LC_MESSAGES LANG; do
		eval "ob_value=\${$ob_name:-}"
		if [ -n "$ob_value" ]; then
			case $ob_value in
			[zZ][hH]*) ob_zh=1 ;;
			esac
			return 0
		fi
	done
}

# ob_say <en-US> <zh-CN>: one line in the chosen language.
ob_say() {
	if [ "$ob_zh" = 1 ]; then
		printf '%s\n' "$2"
	else
		printf '%s\n' "$1"
	fi
}

# ob_fail <en-US> <zh-CN>: the reason on stderr, then exit 1; the exit trap cleans up.
ob_fail() {
	ob_say "$1" "$2" >&2
	exit 1
}

ob_cleanup() {
	# Interrupted between the two moves: put the older application back.
	if [ -n "$ob_previous" ] && [ -d "$ob_previous" ] && [ ! -e "$ob_target" ]; then
		mv "$ob_previous" "$ob_target" 2>/dev/null
	fi
	if [ -n "$ob_staged" ] && [ -d "$ob_staged" ]; then
		rm -rf "$ob_staged"
	fi
	if [ -n "$ob_tmp" ]; then
		rm -rf "$ob_tmp"
	fi
}

ob_usage() {
	ob_say 'Usage: curl -fsSL https://orchbee.com/install.sh | sh -s -- [--desktop] [--no-path] [--version <version>]' \
		'用法：curl -fsSL https://orchbee.com/install.sh | sh -s -- [--desktop] [--no-path] [--version <版本>]'
}

ob_detect_platform() {
	case $(uname -s 2>/dev/null) in
	Darwin) ob_os=macos ;;
	Linux) ob_os=linux ;;
	MINGW* | MSYS* | CYGWIN* | Windows_NT) ob_os=windows ;;
	*) ob_os='' ;;
	esac
	case $(uname -m 2>/dev/null) in
	arm64 | aarch64) ob_cpu=aarch64 ;;
	x86_64 | amd64) ob_cpu=x86_64 ;;
	*) ob_cpu='' ;;
	esac
	if [ "$ob_os" = macos ] && [ "$ob_cpu" = x86_64 ] &&
		[ "$(sysctl -n sysctl.proc_translated 2>/dev/null)" = 1 ]; then
		ob_cpu=aarch64
	fi
	if [ "$ob_os" = windows ]; then
		ob_fail 'On Windows, install Orchbee with the installer from https://orchbee.com. Nothing was installed.' \
			'在 Windows 上，请使用 https://orchbee.com 提供的安装程序安装 Orchbee。未安装任何内容。'
	fi
	if [ -z "$ob_os" ] || [ -z "$ob_cpu" ]; then
		ob_fail "Orchbee has no build for $(uname -s 2>/dev/null) on $(uname -m 2>/dev/null). Nothing was installed." \
			"Orchbee 没有适用于 $(uname -m 2>/dev/null) 上的 $(uname -s 2>/dev/null) 的版本。未安装任何内容。"
	fi
	if [ "$ob_os" = macos ]; then
		# A version this script cannot read is not refused; the install goes on.
		ob_macos_version=$(sw_vers -productVersion 2>/dev/null)
		ob_macos_major=${ob_macos_version%%.*}
		case $ob_macos_major in
		'' | *[!0-9]*) ;;
		*)
			if [ "$ob_macos_major" -lt 13 ]; then
				ob_fail "Orchbee needs macOS 13 or later; this Mac runs macOS $ob_macos_version. Nothing was installed." \
					"Orchbee 需要 macOS 13 或更高版本；这台 Mac 运行的是 macOS ${ob_macos_version}。未安装任何内容。"
			fi
			;;
		esac
	fi
	ob_platform="$ob_os-$ob_cpu"
}

# ob_fetch <url> <file> <byte limit>: curl's exit code; the HTTP status in ob_status.
ob_fetch() {
	ob_status=$(curl --disable --silent --show-error --location --max-redirs 5 \
		--proto "$ob_proto" --proto-redir "$ob_proto" --tlsv1.2 --globoff \
		--connect-timeout 20 --speed-limit 1024 --speed-time 60 --retry 2 \
		--max-filesize "$3" --output "$2" --write-out '%{http_code}' "$1" </dev/null)
}

# ob_download <url> <file> <byte limit> <what>: fails unless the whole answer arrived.
ob_download() {
	if ! ob_fetch "$1" "$2" "$3"; then
		ob_fail "Could not download $4 from $1. Check the connection and run this again. Nothing was installed." \
			"无法从 $1 下载$4。请检查网络后重新运行。未安装任何内容。"
	fi
	case $ob_status in
	200 | 000) ;;
	*)
		ob_fail "$1 answered $ob_status. Nothing was installed." \
			"$1 返回了 ${ob_status}。未安装任何内容。"
		;;
	esac
}

ob_sha256() {
	if command -v sha256sum >/dev/null 2>&1; then
		sha256sum "$1" | cut -d ' ' -f 1
	elif command -v shasum >/dev/null 2>&1; then
		shasum -a 256 "$1" | cut -d ' ' -f 1
	else
		openssl dgst -sha256 -r "$1" | cut -d ' ' -f 1
	fi
}

ob_size() {
	wc -c <"$1" | tr -d ' '
}

# An address the feed names: absolute, or with the test feed only, a path beside the feed.
ob_resolve() {
	case $1 in
	https://*)
		case $1 in
		*[!A-Za-z0-9._~:/%+=@-]*)
			ob_fail 'The feed names an archive address this script does not accept. Nothing was installed.' \
				'发布源给出的归档地址不被接受。未安装任何内容。'
			;;
		esac
		if [ "$ob_proto" != '=https' ]; then
			ob_fail 'A test feed must name its archive beside the feed. Nothing was installed.' \
				'测试发布源必须把归档放在发布源旁边。未安装任何内容。'
		fi
		ob_resolved=$1
		;;
	*)
		case $1 in
		'' | */../* | ../* | */.. | .. | *[!A-Za-z0-9._/-]*)
			ob_fail 'The feed names an archive address this script does not accept. Nothing was installed.' \
				'发布源给出的归档地址不被接受。未安装任何内容。'
			;;
		esac
		if [ -z "${ORCHBEE_INSTALL_SH_FEED:-}" ]; then
			ob_fail 'The feed names its archive by a relative path, which only a test feed may do. Nothing was installed.' \
				'发布源用相对路径给出归档，只有测试发布源可以这样做。未安装任何内容。'
		fi
		ob_resolved="${ob_feed_url%/*}/$1"
		;;
	esac
}

ob_read_feed() {
	ob_feed=$(cat "$ob_tmp/feed.json")
	ob_head="{\"schemaVersion\":1,\"product\":\"orchbee\",\"channel\":\"stable\",\"platform\":\"$ob_platform\",\"version\":\""
	case $ob_feed in
	"$ob_head"*) ;;
	*)
		ob_fail "$ob_feed_url is not a feed document of the stable channel for $ob_platform. Nothing was installed." \
			"$ob_feed_url 不是 $ob_platform 稳定通道的发布源文档。未安装任何内容。"
		;;
	esac
	ob_fields=$(printf '%s\n' "$ob_feed" | sed -n 's/^{"schemaVersion":1,"product":"orchbee","channel":"stable","platform":"[a-z0-9_-]*","version":"\([0-9A-Za-z.-]*\)","buildId":"[A-Za-z0-9._+-]*","manifest":{"sha256":"[0-9a-f]\{64\}"},"archive":{"format":"tar\.gz","url":"\([^"]*\)","size":\([0-9]\{1,10\}\),"sha256":"\([0-9a-f]\{64\}\)"}.*$/\1 \2 \3 \4/p')
	if [ -z "$ob_fields" ]; then
		ob_fail "The feed document at $ob_feed_url names no .tar.gz archive this script can read. Nothing was installed." \
			"$ob_feed_url 的发布源文档没有给出本脚本能读取的 .tar.gz 归档。未安装任何内容。"
	fi
	read -r ob_version ob_archive_url ob_archive_size ob_archive_sha256 <<EOF
$ob_fields
EOF
	if [ -n "$ob_wanted" ] && [ "$ob_wanted" != "$ob_version" ]; then
		ob_fail "The feed names version $ob_version, not $ob_wanted. Nothing was installed." \
			"发布源给出的版本是 ${ob_version}，不是 ${ob_wanted}。未安装任何内容。"
	fi
	if [ "$ob_desktop" = 1 ]; then
		# The window archive in extensions.desktop; a release whose window did not change may name
		# an earlier window version.
		ob_window_fields=$(printf '%s\n' "$ob_feed" | sed -n 's/^.*,"extensions":{"desktop":{"version":"\([0-9A-Za-z.-]*\)","manifest":{"sha256":"[0-9a-f]\{64\}"},"archive":{"format":"zip","url":"\([^"]*\)","size":\([0-9]\{1,10\}\),"sha256":"\([0-9a-f]\{64\}\)"}.*$/\1 \2 \3 \4/p')
		if [ -z "$ob_window_fields" ]; then
			ob_fail "Release $ob_version offers no desktop app for $ob_platform. Nothing was installed." \
				"$ob_version 版本没有提供适用于 $ob_platform 的桌面应用。未安装任何内容。"
		fi
		read -r ob_window_version ob_window_url ob_window_size ob_window_sha256 <<EOF
$ob_window_fields
EOF
	fi
}

ob_fetch_archive() {
	ob_resolve "$ob_archive_url"
	ob_archive_url=$ob_resolved
	ob_archive_name=${ob_archive_url##*/}
	ob_suffix="-$ob_version-$ob_platform.tar.gz"
	ob_stem=${ob_archive_name%"$ob_suffix"}
	case $ob_stem in
	"$ob_archive_name" | '' | *[!A-Za-z0-9_-]*)
		ob_fail "The feed names $ob_archive_name, which is not the archive of $ob_version for $ob_platform. Nothing was installed." \
			"发布源给出的 $ob_archive_name 不是 $ob_platform 的 $ob_version 归档。未安装任何内容。"
		;;
	esac
	ob_say "Downloading Orchbee $ob_version for $ob_platform..." "正在下载适用于 $ob_platform 的 Orchbee $ob_version..."
	ob_download "$ob_archive_url" "$ob_tmp/$ob_archive_name" "$ob_archive_size" "$ob_archive_name"
	if [ "$(ob_size "$ob_tmp/$ob_archive_name")" != "$ob_archive_size" ] ||
		[ "$(ob_sha256 "$ob_tmp/$ob_archive_name")" != "$ob_archive_sha256" ]; then
		ob_fail "$ob_archive_name does not match the size and SHA-256 the feed names. Nothing was installed." \
			"$ob_archive_name 与发布源给出的大小和 SHA-256 不符。未安装任何内容。"
	fi
	mkdir "$ob_tmp/unpacked" || exit 1
	if ! tar -x -z -f "$ob_tmp/$ob_archive_name" -C "$ob_tmp/unpacked" </dev/null; then
		ob_fail "Could not unpack $ob_archive_name. Nothing was installed." \
			"无法解压 ${ob_archive_name}。未安装任何内容。"
	fi
	ob_folder="$ob_tmp/unpacked/$ob_stem-$ob_version-$ob_platform"
	ob_client="$ob_folder/$ob_stem"
	if [ ! -f "$ob_client" ] || [ ! -x "$ob_client" ]; then
		ob_fail "$ob_archive_name holds no runnable $ob_stem. Nothing was installed." \
			"$ob_archive_name 中没有可运行的 ${ob_stem}。未安装任何内容。"
	fi
}

# The window archive the feed names, downloaded and checked before anything is installed: one
# folder `<stem>-desktop-<window version>-<platform>` holding the signed application and its file
# list.
ob_fetch_window() {
	ob_resolve "$ob_window_url"
	ob_window_url=$ob_resolved
	ob_window_name=${ob_window_url##*/}
	ob_window_folder="$ob_stem-desktop-$ob_window_version-$ob_platform"
	if [ "$ob_window_name" != "$ob_window_folder.zip" ]; then
		ob_fail "The feed names $ob_window_name, which is not the desktop app of $ob_window_version for $ob_platform. Nothing was installed." \
			"发布源给出的 $ob_window_name 不是 $ob_platform 的 $ob_window_version 桌面应用。未安装任何内容。"
	fi
	ob_say "Downloading $ob_window_name..." "正在下载 $ob_window_name..."
	ob_download "$ob_window_url" "$ob_tmp/$ob_window_name" "$ob_window_size" "$ob_window_name"
	if [ "$(ob_size "$ob_tmp/$ob_window_name")" != "$ob_window_size" ] ||
		[ "$(ob_sha256 "$ob_tmp/$ob_window_name")" != "$ob_window_sha256" ]; then
		ob_fail "$ob_window_name does not match the size and SHA-256 the feed names. Nothing was installed." \
			"$ob_window_name 与发布源给出的大小和 SHA-256 不符。未安装任何内容。"
	fi
}

# Where the application goes: the test folder; else where a copy already is; else /Applications
# when writable; else ~/Applications.
ob_applications() {
	if [ -n "${ORCHBEE_INSTALL_SH_APPLICATIONS:-}" ]; then
		case $ORCHBEE_INSTALL_SH_APPLICATIONS in
		/*) ob_destination=$ORCHBEE_INSTALL_SH_APPLICATIONS ;;
		*) ob_fail 'ORCHBEE_INSTALL_SH_APPLICATIONS must be an absolute path.' 'ORCHBEE_INSTALL_SH_APPLICATIONS 必须是绝对路径。' ;;
		esac
	elif [ -d "/Applications/$1" ] && [ -w /Applications ]; then
		ob_destination=/Applications
	elif [ -d "$HOME/Applications/$1" ]; then
		ob_destination="$HOME/Applications"
	elif [ -w /Applications ]; then
		ob_destination=/Applications
	else
		ob_destination="$HOME/Applications"
	fi
	mkdir -p "$ob_destination" ||
		ob_fail "Could not create $ob_destination; the command line is installed, the desktop app is not." \
			"无法创建 ${ob_destination}；命令行已安装，桌面应用未安装。"
}

ob_install_desktop() {
	ob_partial='the command line is installed, the desktop app is not'
	ob_partial_zh='命令行已安装，桌面应用未安装'
	mkdir "$ob_tmp/window" || exit 1
	# The system tar reads a zip as well.
	if ! tar -x -f "$ob_tmp/$ob_window_name" -C "$ob_tmp/window" </dev/null; then
		ob_fail "Could not unpack $ob_window_name; $ob_partial." "无法解压 ${ob_window_name}；${ob_partial_zh}。"
	fi
	ob_application=''
	for ob_entry in "$ob_tmp/window/$ob_window_folder"/*.app; do
		if [ -d "$ob_entry" ]; then
			[ -z "$ob_application" ] ||
				ob_fail "$ob_window_name holds more than one application; $ob_partial." "$ob_window_name 中有多个应用；${ob_partial_zh}。"
			ob_application=$ob_entry
		fi
	done
	[ -n "$ob_application" ] ||
		ob_fail "$ob_window_name holds no application; $ob_partial." "$ob_window_name 中没有应用；${ob_partial_zh}。"
	ob_name=${ob_application##*/}
	ob_helper="$ob_application/Contents/Resources/cohort/$ob_stem"
	if [ ! -f "$ob_helper" ] || [ ! -x "$ob_helper" ]; then
		ob_fail "$ob_name holds no runnable $ob_stem; $ob_partial." "$ob_name 中没有可运行的 ${ob_stem}；${ob_partial_zh}。"
	fi
	# The store's copy of this window version, which the application switches to at Update. One
	# orchbee.install document answers; a build without window updates refuses the word as a usage
	# error (exit 2) and answers none.
	ob_answer=$("$ob_helper" --installer-window-v1 </dev/null 2>"$ob_tmp/window.err")
	ob_code=$?
	case $ob_answer in
	*'"schema": "orchbee.install"'*)
		if [ "$ob_code" != 0 ]; then
			ob_error=$(printf '%s\n' "$ob_answer" | sed -n 's/^[[:space:]]*"error": "\([a-z0-9-]*\)".*$/\1/p')
			ob_fail "Could not add $ob_name to the Orchbee store (${ob_error:-exit $ob_code}); $ob_partial." \
				"无法把 $ob_name 加入 Orchbee 存储（${ob_error:-退出码 $ob_code}）；${ob_partial_zh}。"
		fi
		;;
	*)
		if [ "$ob_code" != 2 ]; then
			cat "$ob_tmp/window.err" >&2
			ob_fail "$ob_name's $ob_stem exited $ob_code; $ob_partial." \
				"$ob_name 中的 $ob_stem 以 $ob_code 退出；${ob_partial_zh}。"
		fi
		;;
	esac
	ob_applications "$ob_name"
	ob_target="$ob_destination/$ob_name"
	if [ -e "$ob_target" ] && pgrep -f "$ob_target/Contents/" >/dev/null 2>&1; then
		ob_fail "$ob_target is running. Quit Orchbee and run this again; $ob_partial." \
			"$ob_target 正在运行。请退出 Orchbee 后重新运行；${ob_partial_zh}。"
	fi
	ob_staged="$ob_destination/.$ob_name.install-$$"
	rm -rf "$ob_staged"
	if ! ditto "$ob_application" "$ob_staged" </dev/null ||
		! codesign --verify --deep --strict "$ob_staged" </dev/null >/dev/null 2>&1; then
		ob_fail "Could not copy $ob_name into $ob_destination with its signature intact; $ob_partial." \
			"无法把 $ob_name 完整（含签名）复制到 ${ob_destination}；${ob_partial_zh}。"
	fi
	if [ -e "$ob_target" ]; then
		ob_previous="$ob_destination/.$ob_name.previous-$$"
		mv "$ob_target" "$ob_previous" ||
			ob_fail "Could not replace $ob_target; $ob_partial." "无法替换 ${ob_target}；${ob_partial_zh}。"
	fi
	mv "$ob_staged" "$ob_target" ||
		ob_fail "Could not move $ob_name into $ob_destination; $ob_partial." "无法把 $ob_name 移入 ${ob_destination}；${ob_partial_zh}。"
	ob_staged=''
	if [ -n "$ob_previous" ]; then
		rm -rf "$ob_previous" ||
			ob_say "Could not remove the older copy $ob_previous; delete it by hand." \
				"无法删除旧副本 ${ob_previous}，请手动删除。" >&2
		ob_previous=''
	fi
	ob_say "Installed $ob_name in $ob_destination." "已将 $ob_name 安装到 ${ob_destination}。"
}

ob_main() {
	ob_choose_locale
	ob_desktop=0
	ob_no_path=''
	ob_wanted=''
	while [ $# -gt 0 ]; do
		case $1 in
		--desktop) ob_desktop=1 ;;
		--no-path) ob_no_path='--no-path' ;;
		--version)
			[ $# -ge 2 ] || {
				ob_usage >&2
				exit 2
			}
			ob_wanted=$2
			shift
			;;
		-h | --help)
			ob_usage
			exit 0
			;;
		*)
			ob_usage >&2
			exit 2
			;;
		esac
		shift
	done
	case $ob_wanted in
	*[!0-9A-Za-z.-]* | .* | *.)
		ob_usage >&2
		exit 2
		;;
	esac

	for ob_program in curl tar uname sed cut wc mktemp; do
		command -v "$ob_program" >/dev/null 2>&1 ||
			ob_fail "This script needs $ob_program. Nothing was installed." "本脚本需要 ${ob_program}。未安装任何内容。"
	done
	ob_detect_platform
	if [ "$ob_desktop" = 1 ] && [ "$ob_os" != macos ]; then
		ob_fail "--desktop is not available on $ob_platform yet. Nothing was installed." \
			"--desktop 暂不支持 ${ob_platform}。未安装任何内容。"
	fi

	ob_proto='=https'
	if [ -n "${ORCHBEE_INSTALL_SH_FEED:-}" ]; then
		ob_feed_url=$ORCHBEE_INSTALL_SH_FEED
		case $ob_feed_url in
		https://*) ;;
		http://127.0.0.1:*/*) ob_proto='=http' ;;
		file:///*) ob_proto='=file' ;;
		*) ob_fail 'ORCHBEE_INSTALL_SH_FEED must be an HTTPS, loopback HTTP or file address.' 'ORCHBEE_INSTALL_SH_FEED 必须是 HTTPS、本机 HTTP 或 file 地址。' ;;
		esac
	elif [ -n "$ob_wanted" ]; then
		ob_feed_url="$ob_release_root/v$ob_wanted/orchbee-$ob_wanted-$ob_platform.feed.json"
	else
		ob_feed_url="$ob_feed_root/$ob_platform/feed.json"
	fi

	trap ob_cleanup EXIT
	trap 'exit 129' HUP
	trap 'exit 130' INT
	trap 'exit 143' TERM
	ob_tmp=$(mktemp -d "${TMPDIR:-/tmp}/orchbee-install.XXXXXX") ||
		ob_fail 'Could not create a temporary folder. Nothing was installed.' '无法创建临时文件夹。未安装任何内容。'

	if ! ob_fetch "$ob_feed_url" "$ob_tmp/feed.json" "$ob_feed_limit"; then
		ob_fail "Could not download the feed document from $ob_feed_url. Check the connection and run this again. Nothing was installed." \
			"无法从 $ob_feed_url 下载发布源文档。请检查网络后重新运行。未安装任何内容。"
	fi
	case $ob_status in
	200 | 000) ;;
	404)
		if [ -n "$ob_wanted" ]; then
			ob_fail "Orchbee $ob_wanted has no release for $ob_platform. Nothing was installed." \
				"Orchbee $ob_wanted 没有适用于 $ob_platform 的发布版本。未安装任何内容。"
		fi
		ob_fail "Orchbee has no release for $ob_platform yet. Nothing was installed." \
			"Orchbee 还没有适用于 $ob_platform 的发布版本。未安装任何内容。"
		;;
	*)
		ob_fail "$ob_feed_url answered $ob_status. Nothing was installed." "$ob_feed_url 返回了 ${ob_status}。未安装任何内容。"
		;;
	esac
	ob_read_feed
	ob_fetch_archive
	if [ "$ob_desktop" = 1 ]; then
		ob_fetch_window
	fi

	"$ob_client" install ${ob_no_path:+"$ob_no_path"} </dev/null
	ob_code=$?
	if [ "$ob_code" != 0 ]; then
		ob_say "orchbee install exited $ob_code; see its message above." \
			"orchbee install 以 $ob_code 退出，原因见上方信息。" >&2
		exit "$ob_code"
	fi
	if [ "$ob_desktop" = 1 ]; then
		ob_install_desktop
	fi
	exit 0
}

ob_main "$@"
