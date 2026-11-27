#!/bin/sh
# Installs the Orchbee command line on macOS or Linux from the release the stable channel names,
# and with --desktop the window application too:
#
#   curl -fsSL https://orchbee.com/install.sh | sh
#   curl -fsSL https://orchbee.com/install.sh | sh -s -- [--desktop] [--no-path] [--version <version>]
#
# 1. Takes the platform token from uname: macos-aarch64, macos-x86_64, linux-x86_64 or
#    linux-aarch64. A shell under Rosetta on Apple silicon still installs macos-aarch64.
#    On a Mac older than macOS 13 it stops here, before anything is downloaded.
# 2. Reads that platform's feed document, https://orchbee.com/update/stable/<platform>/feed.json,
#    and its detached signature, https://orchbee.com/update/stable/<platform>/feed.json.sig, or
#    with --version the feed pair of that GitHub release. A platform without a feed is told so in
#    one sentence, and nothing is downloaded or installed. Verifies the Ed25519 signature over
#    `orchbee-update-feed-v1\n` and the exact feed bytes against RELEASE_KEYS (with OpenSSL 3 or
#    Node.js `node:crypto`) before reading the feed or downloading any archive. Without either, it
#    says in one line that the signature could not be checked and goes on over HTTPS, where the
#    signed release.json that `orchbee install` verifies still gates the install.
# 3. Downloads the .tar.gz the feed names, checks its size and SHA-256 against the feed, unpacks
#    it into a temporary folder and runs its `orchbee install`, which verifies the signed
#    release.json, activates the version under ~/.local/share/orchbee, links ~/.local/bin/orchbee,
#    and, when the shell lacks ~/.local/bin, adds the lines that put it on PATH to the shell's
#    startup file, or prints the line where it has none. --no-path is passed on.
# 4. With --desktop on macOS, it first downloads the window archive the feed names in
#    extensions.desktop and checks its size and SHA-256 against the feed; after the install it
#    unpacks it, has the application's bundled Client add that window version to the store with
#    --installer-window, so the application can update itself, then copies the application
#    into /Applications when that folder is writable and otherwise into ~/Applications, replacing an
#    older copy there, and verifies its signature. When the target bundle is already running the
#    same release, nothing is stopped or replaced; when it runs an older release, the new bundle is
#    staged and verified first, the open window is asked to quit gracefully (keeping terminals and
#    Agents running), the bundle is swapped in place and reopened, or, if the window stays open past
#    the wait, the staged version remains ready in the store for Update. A release whose window
#    updates are not switched on yet does not know that word; its application is copied alone. A
#    download through curl carries no quarantine attribute, so the application opens without the
#    Gatekeeper prompt a browser download meets.
#    With --desktop on Linux, the window archive is a .tar.gz, checked the same way; after the
#    install the application's bundled Client adds that window version to the store with
#    --installer-window, which also writes its application menu entry and icons, and nothing is
#    copied elsewhere. An open window of an older version offers Update to switch to it.
#    Either way, the bundled Client of a release up to 0.1.0-beta.24 is asked with
#    --installer-window-v1, the only spelling of that word it knows.
# 5. Removes the temporary folder on every exit, interrupted or not.
#
# Where a verifier exists, the feed signature (here) is checked before the feed is trusted, so an
# unsigned feed is refused, and `orchbee install` then verifies the release directory's signed
# release.json before its contents are trusted. Without a verifier the script falls back to HTTPS
# alone, prints a notice, and runs the downloaded `orchbee` unverified.
# Messages follow LC_ALL, LC_MESSAGES or LANG: zh-CN for any zh value, otherwise en-US.
#
# Two variables exist for tests and rehearsals only:
#   ORCHBEE_INSTALL_SH_FEED          the feed document address in place of orchbee.com's: HTTPS,
#                                    http://127.0.0.1:<port>/…, or file:///…; only with it may the
#                                    feed name its archive by a path relative to the feed or verify
#                                    with TEST_KEYS
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
ob_signature_limit=4096
ob_archive_limit=1073741824

# Compiled Ed25519 public keys from apps/orchbee/src/cli/update/signature.rs.
ob_release_keys='
16bcd2b392d5427892787568853a7ff2d2e40393761c25f187823f2818ec1497
16bf2794f79721de5621c2f0d3c23afc96bc91d5183bfaef71b26bae93c14839
'
ob_test_keys='
02c5eb6b1228c0f555fabd1295d89777920595bae4cfe384f28c4b57edff3518
'

ob_zh=0
ob_openssl=''
ob_node=''
ob_openssl_paths='
/opt/homebrew/opt/openssl@3/bin/openssl
/usr/local/opt/openssl@3/bin/openssl
/opt/homebrew/opt/openssl/bin/openssl
/usr/local/opt/openssl/bin/openssl
/opt/local/bin/openssl
'
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
	elif command -v openssl >/dev/null 2>&1; then
		openssl dgst -sha256 -r "$1" | cut -d ' ' -f 1
	else
		"$ob_node" -e 'const c=require("node:crypto"),fs=require("node:fs");process.stdout.write(c.createHash("sha256").update(fs.readFileSync(0)).digest("hex")+"\n")' <"$1"
	fi
}

ob_size() {
	wc -c <"$1" | tr -d ' '
}

# Selects an Ed25519 verifier: first an OpenSSL binary whose `pkeyutl` supports `-rawin` (on macOS,
# `/usr/bin/openssl` is LibreSSL without `-rawin`, while Homebrew installs keg-only OpenSSL 3), then
# Node.js >= 18 through built-in `node:crypto`. Finding neither is not fatal: ob_verify_feed says so
# and goes on over HTTPS, where `orchbee install` still verifies the signed release.json. Returns 1
# and leaves ob_openssl and ob_node empty when there is none.
ob_find_verifier() {
	ob_openssl=''
	ob_node=''
	ob_path_openssl=$(command -v openssl 2>/dev/null || true)
	for ob_candidate in "$ob_path_openssl" $ob_openssl_paths; do
		[ -n "$ob_candidate" ] && [ -x "$ob_candidate" ] || continue
		if [ -n "$("$ob_candidate" pkeyutl -help 2>&1 | sed -n '/-rawin/p')" ]; then
			ob_openssl=$ob_candidate
			return 0
		fi
	done
	ob_path_node=$(command -v node 2>/dev/null || true)
	if [ -n "$ob_path_node" ] && [ -x "$ob_path_node" ] &&
		"$ob_path_node" -e 'const c=require("node:crypto");process.exit(typeof c.verify==="function"?0:1)' </dev/null >/dev/null 2>&1; then
		ob_node=$ob_path_node
		return 0
	fi
	return 1
}

# Writes raw bytes from an even-length lowercase hexadecimal string $1 into file $2 without
# forking a subshell per byte.
ob_hex_to_raw() {
	ob_rest=$1
	ob_octal=''
	while [ -n "$ob_rest" ]; do
		ob_tail=${ob_rest#??}
		ob_pair=${ob_rest%"$ob_tail"}
		ob_rest=$ob_tail
		ob_byte=$((0x$ob_pair))
		ob_octal="$ob_octal\\0$((ob_byte / 64))$(((ob_byte / 8) % 8))$((ob_byte % 8))"
	done
	printf '%b' "$ob_octal" >"$2"
}

ob_sig_malformed() {
	ob_fail "The feed signature at $ob_feed_url.sig is malformed. Nothing was installed." \
		"$ob_feed_url.sig 的发布源签名格式无效。未安装任何内容。"
}

# Verifies $ob_tmp/feed.json.sig over `orchbee-update-feed-v1\n` + $ob_tmp/feed.json against
# RELEASE_KEYS (and TEST_KEYS only when ORCHBEE_INSTALL_SH_FEED is non-empty) before reading the
# feed or downloading any archive. With no verifier it says so in one line and returns, leaving the
# feed trusted over HTTPS alone; a verifier that finds a bad signature still stops the install.
ob_verify_feed() {
	[ -n "$ob_openssl" ] || [ -n "$ob_node" ] || ob_find_verifier
	ob_feed_bytes=$(ob_size "$ob_tmp/feed.json")
	if [ -z "$ob_feed_bytes" ] || [ "$ob_feed_bytes" -le 0 ] || [ "$ob_feed_bytes" -gt "$ob_feed_limit" ]; then
		ob_fail "The feed document at $ob_feed_url exceeds the size bound. Nothing was installed." \
			"$ob_feed_url 的发布源文档超出大小限制。未安装任何内容。"
	fi
	if [ -z "$ob_openssl" ] && [ -z "$ob_node" ]; then
		ob_say 'The feed signature could not be checked: this machine has neither OpenSSL 3 with Ed25519 support nor Node.js 18 or later. Continuing over HTTPS; orchbee install still verifies the signed release.' \
			'无法校验发布源签名：这台机器既没有支持 Ed25519 的 OpenSSL 3，也没有 Node.js 18 或更高版本。将通过 HTTPS 继续；orchbee install 仍会校验已签名的发布。'
		return 0
	fi
	ob_sig_bytes=$(ob_size "$ob_tmp/feed.json.sig")
	if [ -z "$ob_sig_bytes" ] || [ "$ob_sig_bytes" -le 0 ] || [ "$ob_sig_bytes" -gt "$ob_signature_limit" ]; then
		ob_sig_malformed
	fi
	ob_sig_nonul=$(tr -d '\000' <"$ob_tmp/feed.json.sig" | wc -c | tr -d ' ')
	[ "$ob_sig_nonul" = "$ob_sig_bytes" ] || ob_sig_malformed

	ob_sig_flat=$(tr '\r\n\t' '   ' <"$ob_tmp/feed.json.sig")
	ob_sig_list=$(printf '%s\n' "$ob_sig_flat" | sed -n '
		s/^ *{ *"signatures" *: *\[ *\(.*\) *\] *} *$/\1/p
		t
		s/^ *{ *"signatures" *: *\[ *\(.*\) *\] *, *"extensions" *: *{ *} *} *$/\1/p
		t
		s/^ *{ *"extensions" *: *{ *} *, *"signatures" *: *\[ *\(.*\) *\] *} *$/\1/p
	')
	[ -n "$ob_sig_list" ] || ob_sig_malformed

	printf '%s\n' "$ob_sig_list" | sed 's/} *, *{/}\
{/g' >"$ob_tmp/sig.entries"
	ob_entry_count=$(wc -l <"$ob_tmp/sig.entries" | tr -d ' ')
	case $ob_entry_count in
	1 | 2 | 3 | 4) ;;
	*) ob_sig_malformed ;;
	esac

	: >"$ob_tmp/sig.parsed"
	while IFS= read -r ob_entry; do
		ob_key_id=$(printf '%s\n' "$ob_entry" | sed -n 's/^.*"keyId" *: *"\([0-9a-f]\{16\}\)".*$/\1/p')
		ob_sig_b64=$(printf '%s\n' "$ob_entry" | sed -n 's/^.*"signature" *: *"\([A-Za-z0-9+/]\{85\}[AQgw]==\)".*$/\1/p')
		[ -n "$ob_key_id" ] && [ -n "$ob_sig_b64" ] || ob_sig_malformed
		ob_skeleton=$(printf '%s\n' "$ob_entry" |
			sed 's/"keyId" *: *"[0-9a-f]\{16\}"/K/; s/"algorithm" *: *"ed25519"/A/; s/"signature" *: *"[A-Za-z0-9+/]\{85\}[AQgw]=="/S/' |
			tr -d ' ')
		case $ob_skeleton in
		'{K,A,S}' | '{K,S,A}' | '{A,K,S}' | '{A,S,K}' | '{S,K,A}' | '{S,A,K}') ;;
		*) ob_sig_malformed ;;
		esac
		printf '%s %s\n' "$ob_key_id" "$ob_sig_b64" >>"$ob_tmp/sig.parsed"
	done <"$ob_tmp/sig.entries"

	ob_trusted_keys=$ob_release_keys
	if [ -n "${ORCHBEE_INSTALL_SH_FEED:-}" ]; then
		ob_trusted_keys="$ob_release_keys
$ob_test_keys"
	fi

	{
		printf 'orchbee-update-feed-v1\n'
		cat "$ob_tmp/feed.json"
	} >"$ob_tmp/feed.signed"

	ob_named=0
	while IFS=' ' read -r ob_key_id ob_sig_b64; do
		for ob_pub_hex in $ob_trusted_keys; do
			case $ob_pub_hex in
			*[!0-9a-f]*) continue ;;
			esac
			[ "${#ob_pub_hex}" = 64 ] || continue
			ob_hex_to_raw "$ob_pub_hex" "$ob_tmp/pub.raw"
			ob_candidate_id=$(ob_sha256 "$ob_tmp/pub.raw" | cut -c 1-16)
			[ "$ob_candidate_id" = "$ob_key_id" ] || continue
			ob_named=1
			ob_verified=0
			if [ -n "$ob_openssl" ]; then
				# Ed25519 SubjectPublicKeyInfo DER: 12-byte header 302a300506032b6570032100 + 32-byte raw key.
				ob_hex_to_raw "302a300506032b6570032100$ob_pub_hex" "$ob_tmp/pub.der"
				ob_spki_b64=$("$ob_openssl" base64 -A <"$ob_tmp/pub.der" 2>/dev/null) || continue
				[ -n "$ob_spki_b64" ] || continue
				printf -- '-----BEGIN PUBLIC KEY-----\n%s\n-----END PUBLIC KEY-----\n' "$ob_spki_b64" >"$ob_tmp/pub.pem"
				printf '%s' "$ob_sig_b64" | "$ob_openssl" base64 -d -A >"$ob_tmp/sig.raw" 2>/dev/null || continue
				[ "$(ob_size "$ob_tmp/sig.raw")" = 64 ] || continue
				"$ob_openssl" pkeyutl -verify -pubin -inkey "$ob_tmp/pub.pem" -rawin \
					-in "$ob_tmp/feed.signed" -sigfile "$ob_tmp/sig.raw" </dev/null >/dev/null 2>&1 && ob_verified=1
			else
				"$ob_node" -e '
const crypto = require("node:crypto");
const fs = require("node:fs");
const pubHex = process.argv[1];
const sigB64 = process.argv[2];
const sig = Buffer.from(sigB64, "base64");
if (sig.length !== 64 || sig.toString("base64") !== sigB64) process.exit(1);
const key = crypto.createPublicKey({
	key: Buffer.from("302a300506032b6570032100" + pubHex, "hex"),
	format: "der",
	type: "spki",
});
process.exit(crypto.verify(null, fs.readFileSync(0), key, sig) ? 0 : 1);
' "$ob_pub_hex" "$ob_sig_b64" <"$ob_tmp/feed.signed" >/dev/null 2>&1 && ob_verified=1
			fi
			if [ "$ob_verified" = 1 ]; then
				rm -f "$ob_tmp/feed.signed" "$ob_tmp/sig.entries" "$ob_tmp/sig.parsed" \
					"$ob_tmp/pub.raw" "$ob_tmp/pub.der" "$ob_tmp/pub.pem" "$ob_tmp/sig.raw"
				return 0
			fi
		done
	done <"$ob_tmp/sig.parsed"

	if [ "$ob_named" = 1 ]; then
		ob_fail "The feed signature at $ob_feed_url.sig did not verify. Nothing was installed." \
			"$ob_feed_url.sig 的发布源签名验证失败。未安装任何内容。"
	fi
	ob_fail "The feed at $ob_feed_url is not signed by a trusted Orchbee release key. Nothing was installed." \
		"$ob_feed_url 的发布源未由受信任的 Orchbee 发布密钥签名。未安装任何内容。"
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
		# The window archive in extensions.desktop, a zip on macOS and a .tar.gz on Linux; a release
		# whose window did not change may name an earlier window version.
		ob_window_format=zip
		[ "$ob_os" = linux ] && ob_window_format=tar.gz
		ob_window_format_pattern=$(printf '%s' "$ob_window_format" | sed 's/\./\\./')
		ob_window_fields=$(printf '%s\n' "$ob_feed" | sed -n 's/^.*,"extensions":{"desktop":{"version":"\([0-9A-Za-z.-]*\)","manifest":{"sha256":"[0-9a-f]\{64\}"},"archive":{"format":"'"$ob_window_format_pattern"'","url":"\([^"]*\)","size":\([0-9]\{1,10\}\),"sha256":"\([0-9a-f]\{64\}\)"}.*$/\1 \2 \3 \4/p')
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
	if [ "$ob_window_name" != "$ob_window_folder.$ob_window_format" ]; then
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

# The text $1 as an extended regular expression that matches it literally.
ob_regex() {
	printf '%s' "$1" | sed 's/[][\\.^$*+?(){}|]/\\&/g'
}

# Whether any process of the current user has a command line the extended regular expression $1
# matches, as pgrep -f matches it.
ob_running_matching() {
	ob_uid=$(id -u 2>/dev/null || true)
	if [ -n "$ob_uid" ]; then
		pgrep -u "$ob_uid" -f "$1" >/dev/null 2>&1
		ob_pgrep=$?
		case $ob_pgrep in
		0) return 0 ;;
		1) return 1 ;;
		esac
	fi
	pgrep -f "$1" >/dev/null 2>&1
}

# Whether any process of the current user runs from the path prefix $1.
ob_running_under() {
	ob_running_matching "^$(ob_regex "$1")"
}

# Whether any process of the current user runs from the application bundle $1: its window or a Session.
ob_bundle_running() {
	ob_running_under "$1/Contents/"
}

# Whether the window of the application bundle $1 runs. The window's program lies in Contents/MacOS,
# while a Session's Server, Worker and terminals run from Contents/Resources/cohort.
ob_window_running() {
	ob_running_under "$1/Contents/MacOS/"
}

# Asks the application bundle $1 to quit gracefully, which settles window state and leaves the
# Session's Server, Worker and terminals running. Passed through argv so no path character is parsed
# as AppleScript.
ob_quit_bundle() {
	ob_osascript=$(command -v osascript 2>/dev/null || true)
	[ -n "$ob_osascript" ] || ob_osascript=/usr/bin/osascript
	[ -x "$ob_osascript" ] || return 0
	"$ob_osascript" -e 'on run argv' -e 'tell application (item 1 of argv) to quit' -e 'end run' "$1" </dev/null >/dev/null 2>&1 &
	ob_quit_pid=$!
	ob_quit_timeout=${ob_quit_timeout:-10}
	ob_quit_deadline=$(($(date +%s) + ob_quit_timeout))
	while kill -0 "$ob_quit_pid" 2>/dev/null && [ "$(date +%s)" -lt "$ob_quit_deadline" ]; do
		sleep "${ob_quit_check_sleep:-1}" 2>/dev/null || sleep 1
	done
	if kill -0 "$ob_quit_pid" 2>/dev/null; then
		kill "$ob_quit_pid" 2>/dev/null || true
	fi
	wait "$ob_quit_pid" 2>/dev/null || true
}

ob_open_bundle() {
	ob_open=$(command -v open 2>/dev/null || true)
	[ -n "$ob_open" ] || ob_open=/usr/bin/open
	[ -x "$ob_open" ] || return 0
	"$ob_open" "$1" </dev/null >/dev/null 2>&1 || true
}

# The window installer helper as the window version's bundled Client reads it: releases up to
# 0.1.0-beta.24 answer only its earlier spelling.
ob_window_helper_word() {
	case $ob_window_version in
	0.1.0-beta.[1-9] | 0.1.0-beta.1[0-9] | 0.1.0-beta.2[0-4]) printf '%s\n' --installer-window-v1 ;;
	*) printf '%s\n' --installer-window ;;
	esac
}

# On Linux the window version goes into the store alone: the bundled Client's --installer-window
# installs it under desktop/versions/, makes it active and writes the window entry, the application
# menu entry and its icons, through which the window starts.
ob_install_linux_desktop() {
	if ! tar -x -z -f "$ob_tmp/$ob_window_name" -C "$ob_tmp/window" </dev/null; then
		ob_fail "Could not unpack $ob_window_name; $ob_partial." "无法解压 ${ob_window_name}；${ob_partial_zh}。"
	fi
	ob_helper="$ob_tmp/window/$ob_window_folder/resources/cohort/$ob_stem"
	if [ ! -f "$ob_helper" ] || [ ! -x "$ob_helper" ]; then
		ob_fail "$ob_window_name holds no runnable $ob_stem; $ob_partial." "$ob_window_name 中没有可运行的 ${ob_stem}；${ob_partial_zh}。"
	fi
	ob_answer=$("$ob_helper" "$(ob_window_helper_word)" </dev/null 2>"$ob_tmp/window.err")
	ob_code=$?
	case $ob_answer in
	*'"schema": "orchbee.install"'*) ;;
	*)
		cat "$ob_tmp/window.err" >&2
		ob_fail "The desktop app's $ob_stem exited $ob_code; $ob_partial." \
			"桌面应用中的 $ob_stem 以 $ob_code 退出；${ob_partial_zh}。"
		;;
	esac
	if [ "$ob_code" != 0 ]; then
		ob_error=$(printf '%s\n' "$ob_answer" | sed -n 's/^[[:space:]]*"error": "\([a-z0-9-]*\)".*$/\1/p')
		ob_fail "Could not add the desktop app to the Orchbee store (${ob_error:-exit $ob_code}); $ob_partial." \
			"无法把桌面应用加入 Orchbee 存储（${ob_error:-退出码 $ob_code}）；${ob_partial_zh}。"
	fi
	# The installed version's folder, desktop/versions/<version>; the single window that may run
	# has its program `<stem>-desktop` in one such folder.
	ob_release=$(printf '%s\n' "$ob_answer" | sed -n 's/^[[:space:]]*"releasePath": "\([^"\\]*\)",*$/\1/p')
	if [ -n "$ob_release" ] && ob_running_under "$ob_release/$ob_stem-desktop"; then
		ob_say "Orchbee $ob_window_version is already running." "Orchbee $ob_window_version 已在运行。"
	elif [ -n "$ob_release" ] &&
		ob_running_matching "^$(ob_regex "${ob_release%/*}")/[^/]+/$(ob_regex "$ob_stem")-desktop( |\$)"; then
		ob_say "Added Orchbee $ob_window_version; click Update in the open Orchbee window to switch to it." \
			"已添加 Orchbee ${ob_window_version}；请在已打开的 Orchbee 窗口中点击“更新”切换过去。"
	else
		ob_say "Installed Orchbee $ob_window_version; open it from the application menu." \
			"已安装 Orchbee ${ob_window_version}；请从应用菜单打开。"
	fi
}

ob_install_desktop() {
	ob_partial='the command line is installed, the desktop app is not'
	ob_partial_zh='命令行已安装，桌面应用未安装'
	mkdir "$ob_tmp/window" || exit 1
	if [ "$ob_os" = linux ]; then
		ob_install_linux_desktop
		return 0
	fi
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
	ob_window_stored=0
	ob_answer=$("$ob_helper" "$(ob_window_helper_word)" </dev/null 2>"$ob_tmp/window.err")
	ob_code=$?
	case $ob_answer in
	*'"schema": "orchbee.install"'*)
		if [ "$ob_code" != 0 ]; then
			ob_error=$(printf '%s\n' "$ob_answer" | sed -n 's/^[[:space:]]*"error": "\([a-z0-9-]*\)".*$/\1/p')
			ob_fail "Could not add $ob_name to the Orchbee store (${ob_error:-exit $ob_code}); $ob_partial." \
				"无法把 $ob_name 加入 Orchbee 存储（${ob_error:-退出码 $ob_code}）；${ob_partial_zh}。"
		fi
		ob_window_stored=1
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
	ob_was_running=0
	if [ -d "$ob_target" ] && ob_bundle_running "$ob_target"; then
		ob_was_running=1
	fi
	if [ "$ob_was_running" = 1 ] &&
		[ -f "$ob_target/Contents/Resources/cohort/release.json" ] &&
		[ -f "$ob_application/Contents/Resources/cohort/release.json" ] &&
		cmp -s "$ob_target/Contents/Resources/cohort/release.json" "$ob_application/Contents/Resources/cohort/release.json" &&
		{ [ ! -f "$ob_application/Contents/Info.plist" ] || cmp -s "$ob_target/Contents/Info.plist" "$ob_application/Contents/Info.plist"; } &&
		codesign --verify --deep --strict "$ob_target" </dev/null >/dev/null 2>&1; then
		ob_say "$ob_name $ob_window_version is already running in $ob_destination." \
			"$ob_destination 中的 $ob_name 已在运行 ${ob_window_version}。"
		return 0
	fi
	ob_staged="$ob_destination/.$ob_name.install-$$"
	rm -rf "$ob_staged"
	if ! ditto "$ob_application" "$ob_staged" </dev/null ||
		! codesign --verify --deep --strict "$ob_staged" </dev/null >/dev/null 2>&1; then
		ob_fail "Could not copy $ob_name into $ob_destination with its signature intact; $ob_partial." \
			"无法把 $ob_name 完整（含签名）复制到 ${ob_destination}；${ob_partial_zh}。"
	fi
	if [ "$ob_was_running" = 1 ] || ob_bundle_running "$ob_target"; then
		ob_was_running=1
		# Only a running window is asked to quit. A Session keeps running from the bundle and is left
		# alone, so the bundle stays in place while a Session runs from it.
		if ob_window_running "$ob_target"; then
			ob_quit_bundle "$ob_target"
			ob_tries=${ob_quit_polls:-150}
			while [ "$ob_tries" -gt 0 ] && ob_window_running "$ob_target"; do
				sleep "${ob_quit_poll_sleep:-0.1}" 2>/dev/null || sleep 1
				ob_tries=$((ob_tries - 1))
			done
		fi
		if ob_window_running "$ob_target"; then
			rm -rf "$ob_staged"
			ob_staged=''
			if [ "$ob_window_stored" = 1 ]; then
				ob_say "Staged $ob_name $ob_window_version in the Orchbee store; click Update in the open Orchbee window (or quit and reopen it) to finish switching." \
					"已将 $ob_name $ob_window_version 准备到 Orchbee 存储中；请在已打开的 Orchbee 窗口中点击“更新”（或退出后重新打开）完成切换。"
				return 0
			fi
			ob_fail "$ob_target is running. Quit Orchbee and run this again; $ob_partial." \
				"$ob_target 正在运行。请退出 Orchbee 后重新运行；${ob_partial_zh}。"
		fi
		if ob_bundle_running "$ob_target"; then
			# The window is closed, so it is not asked to quit, and a Session still runs from the bundle.
			rm -rf "$ob_staged"
			ob_staged=''
			if [ "$ob_window_stored" = 1 ]; then
				ob_say "The Orchbee window is closed, but the Session still runs from the old app, so $ob_name was not replaced. $ob_name $ob_window_version is in the Orchbee store: open Orchbee and click Update to finish switching." \
					"Orchbee 窗口已关闭，但会话仍从旧应用运行，因此未替换 ${ob_name}。${ob_name} ${ob_window_version} 已在 Orchbee 存储中：请打开 Orchbee 并点击“更新”完成切换。"
				return 0
			fi
			ob_fail "The Orchbee window is closed, but the Session still runs from the old app, so $ob_name was not replaced. Run orchbee update to put $ob_name $ob_window_version in the Orchbee store, then open Orchbee and click Update; $ob_partial." \
				"Orchbee 窗口已关闭，但会话仍从旧应用运行，因此未替换 ${ob_name}。请运行 orchbee update，把 ${ob_name} ${ob_window_version} 放入 Orchbee 存储，然后打开 Orchbee 并点击“更新”；${ob_partial_zh}。"
		fi
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
	if [ "$ob_was_running" = 1 ]; then
		ob_open_bundle "$ob_target"
		ob_say "Updated $ob_name to $ob_window_version and reopened it (terminals kept running)." \
			"已将 $ob_name 更新到 $ob_window_version 并重新打开（终端继续运行）。"
		return 0
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

	for ob_program in curl tar uname sed cut wc mktemp tr; do
		command -v "$ob_program" >/dev/null 2>&1 ||
			ob_fail "This script needs $ob_program. Nothing was installed." "本脚本需要 ${ob_program}。未安装任何内容。"
	done
	ob_find_verifier
	ob_detect_platform

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
	if ! ob_fetch "$ob_feed_url.sig" "$ob_tmp/feed.json.sig" "$ob_signature_limit"; then
		ob_fail "Could not download the feed signature from $ob_feed_url.sig. Check the connection and run this again. Nothing was installed." \
			"无法从 $ob_feed_url.sig 下载发布源签名。请检查网络后重新运行。未安装任何内容。"
	fi
	case $ob_status in
	200 | 000) ;;
	*)
		ob_fail "$ob_feed_url.sig answered $ob_status. Nothing was installed." \
			"$ob_feed_url.sig 返回了 ${ob_status}。未安装任何内容。"
		;;
	esac
	ob_verify_feed
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
