#!/usr/bin/env bash
# One-time owner setup for the dev F-Droid repository (add-dev-fdroid-repo): generates a random
# password and a PKCS12 repository key (signs the index, never an APK), creates the GitHub
# environment `dev-repo` (deployments from `main` only), stores the three secrets, then saves key
# and password in Bitwarden. Prints no secret, only the public certificate fingerprint to put into
# GS_DEV_REPO_FINGERPRINT (android-native.yml) and the fdroidrepos:// link in the docs.
#   scripts/dev-repo-key.sh [output-dir]              # default ~/guardedsleep-dev-repo-key
#   GS_ROTATE_DEV_REPO_KEY=1 scripts/dev-repo-key.sh   # replace the stored key on purpose
# (a rotation means testers re-add the repository with the new fingerprint)
# Needs admin rights on the repository: a token without them gets HTTP 403, so run it with the
# browser login (`env -u GH_TOKEN -u GITHUB_TOKEN gh auth login --web`, then the same `env -u`
# prefix for this script). Bitwarden: BW_SESSION from the environment or ./.env.local.
set -euo pipefail
umask 077

REPO=ditschi/GuardedSleep
ENV_NAME=dev-repo
ALIAS=guardedsleep-dev-repo
OUT="${1:-$HOME/guardedsleep-dev-repo-key}"
KS="$OUT/guardedsleep-dev-repo.p12"
PWFILE="$OUT/guardedsleep-dev-repo.password"

command -v openssl >/dev/null || { echo "openssl is missing" >&2; exit 1; }
gh auth status >/dev/null 2>&1 || { echo "gh is not logged in (gh auth login)" >&2; exit 1; }
mise exec -- keytool -help >/dev/null 2>&1 || { echo "keytool missing (mise install java)" >&2; exit 1; }
mkdir -p "$OUT"

# A new key changes the repository fingerprint (testers must re-add the repository), so a second run
# never creates one by accident: set GS_ROTATE_DEV_REPO_KEY=1 to replace the stored key on purpose.
if [ ! -e "$KS" ] && [ "${GS_ROTATE_DEV_REPO_KEY:-0}" != "1" ] &&
  gh secret list --env "$ENV_NAME" --repo "$REPO" 2>/dev/null | grep -q '^GS_DEV_REPO_KEYSTORE_B64'; then
  echo "The dev repository key is already set up in GitHub ($ENV_NAME). Nothing to do." >&2
  echo "To replace it on purpose: GS_ROTATE_DEV_REPO_KEY=1 bash $0" >&2
  rmdir "$OUT" 2>/dev/null || true
  exit 1
fi

# The password lives only in this process, the 0600 file and the GitHub secret.
if [ -e "$KS" ] && [ -s "$PWFILE" ]; then
  # A previous run created the key and stopped later: reuse it, never make a second one.
  echo "1/5 Reusing the existing key ($KS)"
  GS_DEV_REPO_PW="$(cat "$PWFILE")"
  export GS_DEV_REPO_PW
  mise exec -- keytool -list -storetype PKCS12 -keystore "$KS" -alias "$ALIAS" \
    -storepass:env GS_DEV_REPO_PW >/dev/null 2>&1 || { echo "$KS does not open with $PWFILE" >&2; exit 1; }
elif [ -e "$KS" ]; then
  echo "$KS exists without its password file; refusing to overwrite a key" >&2
  exit 1
else
  GS_DEV_REPO_PW="$(openssl rand -base64 32 | tr -d '\n')"
  export GS_DEV_REPO_PW
  printf '%s\n' "$GS_DEV_REPO_PW" > "$PWFILE"
  echo "1/5 Creating the key ($KS)"
  # RSA, not EC: fdroidserver signs the legacy index-v1.jar with jarsigner and SHA1withRSA, which an
  # EC key cannot do (clients that read index-v1 would get an unsigned file).
  mise exec -- keytool -genkeypair -storetype PKCS12 -keyalg RSA -keysize 4096 \
    -validity 3650 -alias "$ALIAS" -dname "CN=GuardedSleep Dev Repository" -keystore "$KS" \
    -storepass:env GS_DEV_REPO_PW -keypass:env GS_DEV_REPO_PW >/dev/null 2>&1
fi

echo "2/5 Environment $ENV_NAME (branch main only)"
if gh api "repos/$REPO/environments/$ENV_NAME" >/dev/null 2>&1; then
  echo "    exists; check in the web UI that its deployment branches are limited to main"
else
  # Creating an environment needs admin rights on the repository (fine-grained token:
  # Administration read and write). https://docs.github.com/rest/deployments/environments
  if ! gh api -X PUT "repos/$REPO/environments/$ENV_NAME" \
    -F 'deployment_branch_policy[protected_branches]=false' \
    -F 'deployment_branch_policy[custom_branch_policies]=true' >/dev/null; then
    echo "Cannot create the environment with this token. Create it in the browser:" >&2
    echo "  https://github.com/$REPO/settings/environments/new  (name $ENV_NAME," >&2
    echo "  deployment branches: selected, rule main), then run this script again." >&2
    exit 1
  fi
  gh api -X POST "repos/$REPO/environments/$ENV_NAME/deployment-branch-policies" \
    -f name=main -f type=branch >/dev/null
fi

echo "3/5 Storing the secrets"
base64 -w0 "$KS" | gh secret set GS_DEV_REPO_KEYSTORE_B64 --env "$ENV_NAME" --repo "$REPO"
printf '%s' "$GS_DEV_REPO_PW" | gh secret set GS_DEV_REPO_KEYSTORE_PASSWORD --env "$ENV_NAME" --repo "$REPO"
gh secret set GS_DEV_REPO_KEY_ALIAS --env "$ENV_NAME" --repo "$REPO" --body "$ALIAS"

echo "4/5 Check"
gh secret list --env "$ENV_NAME" --repo "$REPO"
# The fingerprint is public (the SHA-256 of the signing certificate, upper case without colons, the
# format of fdroidrepos:// links and of F-Droid clients).
CERT_FP="$(mise exec -- keytool -exportcert -storetype PKCS12 -keystore "$KS" -alias "$ALIAS" \
  -storepass:env GS_DEV_REPO_PW 2>/dev/null | openssl x509 -inform DER -noout -fingerprint -sha256 |
  cut -d= -f2 | tr -d ':')"
echo "Repository certificate SHA-256 (put into GS_DEV_REPO_FINGERPRINT and the docs): $CERT_FP"
unset GS_DEV_REPO_PW

echo "5/5 Saving key and password in Bitwarden"
# BW_SESSION comes from the caller's environment or ./.env.local (run from the repo root).
if [ -z "${BW_SESSION:-}" ] && [ -f .env.local ]; then
  set -a
  # shellcheck disable=SC1091 # the owner's local file, not part of the repo
  . ./.env.local
  set +a
fi
if [ -n "${BW_SESSION:-}" ] && command -v bw >/dev/null && command -v jq >/dev/null; then
  export BW_SESSION
  bw sync >/dev/null
  bw get template item |
    jq --arg pw "$(cat "$PWFILE")" --arg ks "$(base64 -w0 "$KS")" --arg alias "$ALIAS" \
      '.type = 1 | .name = "GuardedSleep dev repository key" | .folderId = null
       | .login = {username: $alias, password: $pw, uris: []}
       | .notes = "Dev F-Droid repository index signing key (PKCS12, base64). Restore: base64 -d > guardedsleep-dev-repo.p12\n\n" + $ks' |
    bw encode | bw create item >/dev/null
  rm -f "$KS" "$PWFILE"
  rmdir "$OUT" 2>/dev/null || true
  echo
  echo "Done. Bitwarden item \"GuardedSleep dev repository key\" holds password and key; local copies removed."
else
  echo
  echo "Bitwarden skipped (no BW_SESSION, bw or jq). Move $KS and $PWFILE into your password manager, then delete $OUT."
fi
