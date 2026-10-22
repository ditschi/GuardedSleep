package org.guardedsleep.buildlogic

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
}
