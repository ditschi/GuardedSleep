package org.guardedsleep.app.config

import org.guardedsleep.app.platform.BuildConfig
import uniffi.guardedsleep_core.InviteMailbox
import uniffi.guardedsleep_core.RelayCandidate
import uniffi.guardedsleep_core.pickRelays

/**
 * The curated public Nostr relays (`data/nostr-relays.json`, generated into [PublicLists]) a baby
 * names in its invite. The choice itself is the core's: three relays of distinct operators, derived
 * deterministically from the pairing key. This object only hands the shipped list over.
 */
object PublicRelays {
    /**
     * Release gate (alpha-connection-model task 3.6, security review of 3.5, M7): a release build
     * must not add `n:` entries to invites until the disclosure and the off switch exist (tasks 5.3
     * and 6.2, which flip this). On for debug builds (development, the emulator gate), off for
     * release. This is the only switch; [enabledFor] keeps it testable for the release value.
     */
    fun enabledFor(debugBuild: Boolean): Boolean = debugBuild

    val ENABLED: Boolean = enabledFor(BuildConfig.DEBUG)

    /** Relays an invite gets from the shipped list. */
    const val PICK_COUNT = 3

    fun candidates(list: List<PublicRelay> = PublicLists.nostrRelays): List<RelayCandidate> =
        list.map {
            RelayCandidate(it.host, it.operator, it.hoster)
        }

    /** The `nostr` mailboxes for an invite with [psk]; none while the release gate is off. */
    fun forInvite(
        psk: ByteArray,
        enabled: Boolean = ENABLED,
        list: List<PublicRelay> = PublicLists.nostrRelays,
        parked: List<String> = emptyList(),
    ): List<InviteMailbox> =
        if (!enabled) emptyList()
        else pickRelays(psk, candidates(list), parked, PICK_COUNT.toUInt())
}
