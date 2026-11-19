## MODIFIED Requirements

### Requirement: Product identity

The product SHALL be named "GuardedSleep – Privacy Baby Monitor" (shortened to "GuardedSleep Baby Monitor" where a store limits the title). Engineering names SHALL be `GuardedSleep`, the npm scope `@guardedsleep`, Docker images `guardedsleep-*`, and the Android application id `org.guardedsleep.app`, with `org.guardedsleep.app.alpha` for alpha builds and `org.guardedsleep.app.dev` for dev (debug) builds. Dev builds SHALL show a launcher label and icon that set them apart from alpha and release builds, also when the launcher shows themed icons.

#### Scenario: Alpha build installed next to a later release

- **WHEN** an alpha build and a later release build are installed on one phone
- **THEN** they do not replace each other, because the alpha uses its own application id

#### Scenario: Dev build installed next to an alpha build

- **WHEN** a tester has the alpha release installed and installs a dev build from `main`
- **THEN** both apps stay installed with their own pairings, and the launcher shows them as two distinguishable apps

### Requirement: Derived Android version code

The Android `versionCode` of release builds SHALL be derived from `versionName` in `version.properties` and SHALL NOT be edited by hand.

#### Scenario: New alpha build

- **WHEN** `versionName` changes from `0.1.0-alpha.1` to `0.1.0-alpha.2`
- **THEN** the build computes a higher `versionCode` without a manual edit

#### Scenario: Version name outside the supported form

- **WHEN** `versionName` is `1.0.0-snapshot`
- **THEN** the build fails

## ADDED Requirements

### Requirement: Dev repository job isolation

The job that builds the dev F-Droid repository SHALL NOT have access to the dev APK signing key or a token that can write to the repository or its releases, and SHALL install its tools only from hash-pinned packages.

#### Scenario: Compromised repository tool

- **WHEN** a dependency of the repository tool is compromised
- **THEN** it cannot sign an APK with the dev APK key or change the `android-dev` pre-release, because neither that key nor a write token is present in that job, and an installed dev build rejects any update signed with another key

### Requirement: Commit-derived dev version

Dev builds SHALL derive their version from the committer time and hash of the commit they are built from: `versionName` SHALL be the base `versionName` plus `dev.<UTC committer time as YYYYMMDDHHMMSS>` as a SemVer pre-release part and `+<sha8>` as build metadata, and `versionCode` SHALL be the seconds from 2026-01-01T00:00:00Z to the committer time. The same commit SHALL always produce the same dev version.

#### Scenario: Later commit on main

- **WHEN** a commit merged to `main` at 2026-11-19 14:30:12 UTC follows one merged at 2026-11-18 09:00:00 UTC
- **THEN** its dev build has the higher `versionCode`, and its `versionName` is `0.1.0-alpha.1.dev.20261119143012+<sha8>` for the base `0.1.0-alpha.1`

#### Scenario: Old branch rebased onto main

- **WHEN** a commit authored a week ago is rebased and merged onto `main` today
- **THEN** its dev `versionCode` is computed from today's committer time and is higher than the previous dev build's, so F-Droid clients offer it as an update

#### Scenario: Build without commit information

- **WHEN** a dev build runs without commit information, for example from an IDE
- **THEN** it gets `versionCode` 1 and a `versionName` that says the commit is unknown

#### Scenario: Release build unaffected

- **WHEN** the release build is assembled from the same commit
- **THEN** its `versionName` and `versionCode` follow `version.properties` and the version code format, without the dev part

### Requirement: Dev F-Droid repository

Every dev build published from `main` SHALL also be published to an F-Droid repository at `https://ditschi.github.io/GuardedSleep/fdroid/dev/repo`, whose index is signed with a key used for nothing else. The repository SHALL list only the dev application id, keep the three most recent dev builds with one entry per `versionCode`, and mark the app as an unreviewed test build. Its URL, index key and application id SHALL NOT be reused for any release channel.

#### Scenario: Tester updates with Neo Store

- **WHEN** a tester has added the dev repository to Neo Store and a new commit on `main` passes the emulator gate
- **THEN** Neo Store offers the new dev build as an update, and installing it keeps the pairing

#### Scenario: Release channel added later

- **WHEN** the project later publishes releases through IzzyOnDroid or an own stable F-Droid repository
- **THEN** a tester can keep the dev repository and the release source side by side, and neither offers its app as an update of the other

#### Scenario: Same commit published twice

- **WHEN** the dev build of a commit is published again by a re-run or a manual dispatch
- **THEN** the repository lists that `versionCode` once, next to the two older builds

#### Scenario: Repository key missing

- **WHEN** the dev repository key secret is not set in the `dev-repo` environment
- **THEN** the repository job fails with a message naming the missing secret, the published repository is not replaced by an unsigned one, and the `android-dev` pre-release is still updated

### Requirement: Dev pre-release title

The rolling `android-dev` pre-release SHALL be titled with the full dev `versionName` of the build it holds.

#### Scenario: New dev build published

- **WHEN** a new dev build replaces the asset of `android-dev`
- **THEN** the release title changes to the new `versionName`
