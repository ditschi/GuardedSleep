package org.guardedsleep.buildlogic

import java.time.Instant
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter

/** Version of a dev (debug) build; [warning] is set when the fallback was used. */
data class DevVersion(val versionName: String, val versionCode: Int, val warning: String? = null)

/**
 * Derives the Play `versionCode` from `versionName` (version.properties), so it is never edited by
 * hand and always orders like the names do.
 *
 * `major * 10_000_000 + minor * 100_000 + patch * 1_000 + stage * 100 + n`
 *
 * where `stage` is alpha = 1, beta = 2, rc = 3 and a release without suffix = 9 (with `n` = 0).
 * Example: `0.1.0-alpha.1` is 100_101, `0.1.0` is 100_900. Limits: minor, patch and `n` below 100,
 * major up to 209 (the result must fit the 2_100_000_000 ceiling of Google Play).
 */
object AndroidVersion {
    private val pattern =
        Regex("""(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-(alpha|beta|rc)\.([1-9]\d*))?""")
    private val stages = mapOf("alpha" to 1, "beta" to 2, "rc" to 3)
    private const val RELEASE_STAGE = 9
    private const val MAX_MAJOR = 209
    private const val MAX_FIELD = 99

    fun versionCode(versionName: String): Int {
        val m =
            pattern.matchEntire(versionName)
                ?: error(
                    "versionName '$versionName' is not MAJOR.MINOR.PATCH or " +
                        "MAJOR.MINOR.PATCH-(alpha|beta|rc).N"
                )
        val major = m.groupValues[1].toInt()
        val minor = m.groupValues[2].toInt()
        val patch = m.groupValues[3].toInt()
        val stage = m.groupValues[4].takeIf { it.isNotEmpty() }?.let { stages.getValue(it) }
        val n = m.groupValues[5].takeIf { it.isNotEmpty() }?.toInt() ?: 0
        require(major <= MAX_MAJOR) { "major $major above $MAX_MAJOR in '$versionName'" }
        require(minor <= MAX_FIELD && patch <= MAX_FIELD && n <= MAX_FIELD) {
            "minor, patch and pre-release number must be at most $MAX_FIELD in '$versionName'"
        }
        return major * 10_000_000 +
            minor * 100_000 +
            patch * 1_000 +
            (stage ?: RELEASE_STAGE) * 100 +
            n
    }

    /**
     * 2026-01-01T00:00:00Z: dev version codes count seconds from here (Play ceiling lasts to
     * ~2092).
     */
    const val DEV_EPOCH_BASE = 1_767_225_600L
    private const val PLAY_CEILING = 2_100_000_000L
    private val sha8Pattern = Regex("[0-9a-f]{8}")
    private val timestamp = DateTimeFormatter.ofPattern("yyyyMMddHHmmss").withZone(ZoneOffset.UTC)

    /**
     * Dev (debug) builds: `<base><sep>dev.<yyyyMMddHHmmss UTC committer time>+<sha8>` and a code of
     * seconds since [DEV_EPOCH_BASE], so every later commit on `main` orders higher (F-Droid
     * clients update by code). `sep` is `.` when [base] already has a pre-release part, `-`
     * otherwise. Missing or invalid input (no commit info, an epoch before 2026, a code above the
     * Play ceiling) never fails the build: code 1, `dev.0+unknown`, and a [DevVersion.warning].
     */
    fun devVersion(base: String, committerEpoch: String?, commit: String?): DevVersion {
        val sep = if ('-' in base) "." else "-"
        val epoch = committerEpoch?.takeIf { it.length in 1..12 && it.all(Char::isDigit) }?.toLong()
        val sha = commit?.take(8)?.takeIf { sha8Pattern.matches(it) }
        val code = epoch?.minus(DEV_EPOCH_BASE)
        if (epoch == null || sha == null || code == null || code < 1 || code > PLAY_CEILING) {
            return DevVersion(
                versionName = "$base${sep}dev.0+unknown",
                versionCode = 1,
                warning =
                    "dev version falls back to versionCode 1: need GS_GIT_COMMIT_EPOCH (digits, " +
                        "2026-01-01 or later) and GS_GIT_COMMIT (hex); got " +
                        "epoch='${committerEpoch.orEmpty()}', commit='${commit.orEmpty()}'",
            )
        }
        return DevVersion(
            versionName = "$base${sep}dev.${timestamp.format(Instant.ofEpochSecond(epoch))}+$sha",
            versionCode = code.toInt(),
        )
    }
}
