#!/usr/bin/env bash
# Builds the signed dev F-Droid repository (add-dev-fdroid-repo, design §4 and §6): the new dev APK
# plus up to two older builds taken from the live, signature-verified index, indexed and signed by
# `fdroid update`. The dev-repo job of .github/workflows/android-native.yml calls this; it also runs
# locally against a throwaway key (docs/contributing/development.md, "Dev APK").
#
#   scripts/dev-fdroid-repo.sh <re-signed-dev.apk> <work-dir>
#
# Result: <work-dir>/site holds the Pages site root, the repository is <work-dir>/site/fdroid/dev/repo.
# The keystore, config.yml (passwords) and fdroid's working tree stay outside the site and are deleted.
#
# Environment (secrets come from the job, never from arguments):
#   GS_DEV_REPO_KEYSTORE            path to the PKCS12 repository key (required)
#   GS_DEV_REPO_KEYSTORE_PASSWORD   its password, also the key password (PKCS12 has one) (required)
#   GS_DEV_REPO_KEY_ALIAS           key alias (required)
#   GS_DEV_REPO_FINGERPRINT         pinned SHA-256 of the repository certificate, hex, colons optional
#                                   (required; the placeholder of the workflow is refused)
#   GS_DEV_REPO_URL                 public repository URL (default https://ditschi.github.io/GuardedSleep/fdroid/dev/repo)
#   GS_DEV_REPO_PREVIOUS_URL        where to fetch the previous index and APKs (default GS_DEV_REPO_URL;
#                                   a file:// URL works for local runs)
#   FDROID, APKSIGNER, AAPT2        tools; default `fdroid` on PATH and the build-tools of ANDROID_HOME
#   GS_DEV_REPO_ICON                png for the repository icon (default apps/web/public/pwa-512.png)
set -euo pipefail
umask 077

APP_ID=org.guardedsleep.app.dev
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
APK="${1:?usage: dev-fdroid-repo.sh <re-signed-dev.apk> <work-dir>}"
WORK="${2:?usage: dev-fdroid-repo.sh <re-signed-dev.apk> <work-dir>}"
REPO_URL="${GS_DEV_REPO_URL:-https://ditschi.github.io/GuardedSleep/fdroid/dev/repo}"
PREV_URL="${GS_DEV_REPO_PREVIOUS_URL:-$REPO_URL}"
FDROID="${FDROID:-fdroid}"
BUILD_TOOLS="${ANDROID_HOME:-}/build-tools/36.0.0"
APKSIGNER="${APKSIGNER:-$BUILD_TOOLS/apksigner}"
AAPT2="${AAPT2:-$BUILD_TOOLS/aapt2}"
ICON="${GS_DEV_REPO_ICON:-$ROOT/apps/web/public/pwa-512.png}"
KEEP_PREVIOUS=2
JAVA_EN=(-J-Duser.language=en -J-Duser.country=US)

die() {
	echo "::error::$*" >&2
	exit 1
}
warn() { echo "::warning::$*" >&2; }

for var in GS_DEV_REPO_KEYSTORE GS_DEV_REPO_KEYSTORE_PASSWORD GS_DEV_REPO_KEY_ALIAS GS_DEV_REPO_FINGERPRINT; do
	[[ -n "${!var:-}" ]] || die "$var is not set (see docs/contributing/development.md, Dev APK)"
done
PINNED="$(printf '%s' "$GS_DEV_REPO_FINGERPRINT" | tr -d ':' | tr 'a-f' 'A-F')"
[[ "$PINNED" =~ ^[0-9A-F]{64}$ ]] ||
	die "GS_DEV_REPO_FINGERPRINT is not a SHA-256 fingerprint (the owner sets it from scripts/dev-repo-key.sh, task 4.1)"
[[ -f "$APK" ]] || die "APK $APK not found"
[[ -f "$GS_DEV_REPO_KEYSTORE" ]] || die "keystore $GS_DEV_REPO_KEYSTORE not found"
for tool in jq curl unzip jarsigner keytool "$FDROID" "$APKSIGNER" "$AAPT2"; do
	command -v "$tool" >/dev/null || die "$tool is missing"
done

# sha256 of the signing certificate of an APK, upper-case hex.
apk_cert_sha256() {
	"$APKSIGNER" verify --print-certs "$1" |
		sed -n 's/^Signer #1 certificate SHA-256 digest: //p' | head -n1 | tr 'a-f' 'A-F'
}

badging="$("$AAPT2" dump badging "$APK")"
pkg_line="$(printf '%s\n' "$badging" | grep -m1 '^package:')"
[[ "$pkg_line" == *"name='$APP_ID'"* ]] || die "$APK has not the application id $APP_ID: $pkg_line"
CODE="$(printf '%s' "$pkg_line" | sed -n "s/.*versionCode='\([0-9]*\)'.*/\1/p")"
[[ "$CODE" =~ ^[0-9]+$ ]] || die "no versionCode in: $pkg_line"
NEW_CERT="$(apk_cert_sha256 "$APK")"
[[ "$NEW_CERT" =~ ^[0-9A-F]{64}$ ]] || die "$APK is not signed (apksigner verify failed)"
echo "New build: $APP_ID versionCode $CODE, signer $NEW_CERT"

rm -rf "$WORK"
FD="$WORK/fdroid"
SITE="$WORK/site"
mkdir -p "$FD/repo" "$FD/metadata" "$SITE" "$WORK/tmp"
trap 'rm -rf "$WORK/tmp" "$WORK/keystore.p12" "$FD/config.yml"' EXIT

# --- previous builds (design §4): the live entry.jar must verify and carry the pinned certificate,
# and everything after it hangs on hashes of that signed index. Any failure means a fresh start.
fetch_previous() {
	local t="$WORK/tmp" cert out
	curl -fsSL --retry 2 --max-time 60 -o "$t/entry.jar" "$PREV_URL/entry.jar" || {
		echo "no live entry.jar at $PREV_URL"
		return 1
	}
	# https://docs.oracle.com/en/java/javase/21/docs/specs/man/jarsigner.html (-verify). English
	# output is forced because the checks below read it. `-strict` is not used, although the design
	# names it: it turns warnings into errors, and every entry.jar from fdroidserver has them (a
	# self-signed chain, a signer certificate that expires, and "internal inconsistencies" of the
	# apksigner v1 layout). Trust comes from the exit status (bad or missing signatures), the
	# absence of unsigned entries and the pinned certificate fingerprint below.
	out="$(jarsigner "${JAVA_EN[@]}" -verify "$t/entry.jar" 2>&1)" || {
		echo "$out"
		return 1
	}
	[[ "$out" == *"jar verified"* ]] || {
		echo "$out"
		return 1
	}
	[[ "$out" != *"contains unsigned entries"* ]] || {
		echo "entry.jar has unsigned entries"
		return 1
	}
	out="$(keytool "${JAVA_EN[@]}" -printcert -jarfile "$t/entry.jar" 2>&1)" || {
		echo "$out"
		return 1
	}
	[[ "$(printf '%s\n' "$out" | grep -c '^Signer #')" == "1" ]] || {
		echo "entry.jar must have exactly one signer"
		return 1
	}
	cert="$(printf '%s\n' "$out" | sed -n 's/^[[:space:]]*SHA256: //p' | head -n1 | tr -d ':' | tr 'a-f' 'A-F')"
	[[ "$cert" == "$PINNED" ]] || {
		echo "entry.jar certificate $cert is not the pinned $PINNED"
		return 1
	}

	local index_name index_sha index_size
	unzip -p "$t/entry.jar" entry.json >"$t/entry.json"
	index_name="$(jq -er '.index.name' "$t/entry.json")"
	index_sha="$(jq -er '.index.sha256' "$t/entry.json")"
	[[ "$index_name" == "/index-v2.json" ]] || {
		echo "unexpected index name $index_name"
		return 1
	}
	curl -fsSL --retry 2 --max-time 60 -o "$t/index-v2.json" "$PREV_URL/index-v2.json"
	[[ "$(sha256sum "$t/index-v2.json" | cut -d' ' -f1)" == "$index_sha" ]] || {
		echo "index-v2.json does not match the signed entry.json"
		return 1
	}
	index_size="$(jq -er '.index.size' "$t/entry.json")"
	[[ "$(wc -c <"$t/index-v2.json")" == "$index_size" ]] || {
		echo "index-v2.json size differs from entry.json"
		return 1
	}

	# The newest older codes, one entry per code (a re-run of this commit has the same code and is
	# replaced by the new build; fdroidserver would list both).
	jq -r --arg id "$APP_ID" --argjson code "$CODE" --argjson n "$KEEP_PREVIOUS" '
		[.packages[$id].versions[]? | {code: (.manifest.versionCode), name: .file.name, sha: .file.sha256}
		 | select(.code < $code)]
		| unique_by(.code) | sort_by(-.code) | .[:$n][] | "\(.code) \(.sha) \(.name)"' \
		"$t/index-v2.json" >"$t/previous.txt"

	local prev_code prev_sha prev_name dest
	while read -r prev_code prev_sha prev_name; do
		[[ "$prev_name" == "/${APP_ID}_${prev_code}.apk" ]] || {
			echo "unexpected APK name $prev_name for code $prev_code"
			return 1
		}
		dest="$FD/repo/${APP_ID}_${prev_code}.apk"
		curl -fsSL --retry 2 --max-time 300 -o "$dest" "$PREV_URL$prev_name"
		[[ "$(sha256sum "$dest" | cut -d' ' -f1)" == "$prev_sha" ]] || {
			echo "$prev_name does not match the signed index"
			return 1
		}
		[[ "$(apk_cert_sha256 "$dest")" == "$NEW_CERT" ]] || {
			echo "$prev_name is not signed with the dev APK certificate"
			return 1
		}
		echo "Keeping previous build versionCode $prev_code"
	done <"$t/previous.txt"
}

if ! msg="$(fetch_previous 2>&1)"; then
	warn "Starting the dev repository fresh: ${msg//$'\n'/ }"
	rm -f "$FD"/repo/*.apk
else
	printf '%s\n' "$msg"
fi

cp "$APK" "$FD/repo/${APP_ID}_${CODE}.apk"

# --- fdroid config and metadata (design §6)
cp "$GS_DEV_REPO_KEYSTORE" "$WORK/keystore.p12"
cp "$ICON" "$FD/repo-icon.png"
# repo_url must be the public URL (it is written into the index and the QR code). The passwords
# sit in this 0600 file only until the end of the script. Keys: https://f-droid.org/docs/Build_Metadata_Reference/
# and fdroidserver examples/config.yml (v2.4.5).
{
	echo "repo_url: $REPO_URL"
	echo "repo_name: GuardedSleep Dev (unreviewed test builds)"
	echo "repo_icon: repo-icon.png"
	echo "repo_description: >-"
	echo "  Unreviewed, debuggable test builds of GuardedSleep from the main branch, for testers. Not a release."
	echo "repo_keyalias: $GS_DEV_REPO_KEY_ALIAS"
	echo "keystore: $WORK/keystore.p12"
	printf 'keystorepass: "%s"\n' "$GS_DEV_REPO_KEYSTORE_PASSWORD"
	printf 'keypass: "%s"\n' "$GS_DEV_REPO_KEYSTORE_PASSWORD"
	echo "keydname: CN=GuardedSleep Dev Repository"
	echo "archive_older: 0"
	[[ -z "${ANDROID_HOME:-}" ]] || echo "sdk_path: $ANDROID_HOME"
} >"$FD/config.yml"
chmod 600 "$FD/config.yml"

cat >"$FD/metadata/$APP_ID.yml" <<'EOF'
# Written by scripts/dev-fdroid-repo.sh; not a release (add-dev-fdroid-repo).
Categories:
  - Multimedia
License: AGPL-3.0-or-later
SourceCode: https://github.com/ditschi/GuardedSleep
IssueTracker: https://github.com/ditschi/GuardedSleep/issues
Name: GuardedSleep Dev
Summary: Unreviewed test build from main, not a release
Description: |-
  Debuggable test build of GuardedSleep, the privacy baby monitor, built from the latest commit on
  the main branch after it passed the emulator gate. It installs next to the alpha and release apps
  and has its own pairing. Updates are offered for every new commit. Not reviewed, not for
  everyday use.
EOF

(cd "$FD" && "$FDROID" update --verbose)

# --- result checks before anything is published
expected="$(find "$FD/repo" -maxdepth 1 -name "${APP_ID}_*.apk" | sed -n "s|.*${APP_ID}_\([0-9]*\)\.apk|\1|p" | sort -n | paste -sd' ')"
listed="$(jq -r --arg id "$APP_ID" '[.packages[$id].versions[].manifest.versionCode] | sort | join(" ")' "$FD/repo/index-v2.json")"
[[ "$expected" == "$listed" ]] ||
	die "index-v2.json lists versionCodes [$listed], the repository files are [$expected]"
[[ -f "$FD/repo/entry.jar" ]] || die "fdroid update wrote no signed entry.jar"
fresh="$(keytool "${JAVA_EN[@]}" -printcert -jarfile "$FD/repo/entry.jar" | sed -n 's/^[[:space:]]*SHA256: //p' | head -n1 | tr -d ':' | tr 'a-f' 'A-F')"
[[ "$fresh" == "$PINNED" ]] ||
	die "the new index is signed by $fresh, not by the pinned $PINNED: GS_DEV_REPO_FINGERPRINT does not belong to the repository key"

# Only the repository goes to Pages: no config, keystore, metadata or fdroid working data.
mkdir -p "$SITE/fdroid/dev"
cp -r "$FD/repo" "$SITE/fdroid/dev/repo"
echo "Repository ready: $SITE/fdroid/dev/repo (versionCodes: $listed)"
