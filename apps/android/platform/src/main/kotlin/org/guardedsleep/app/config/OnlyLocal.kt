package org.guardedsleep.app.config

import java.net.URI
import org.guardedsleep.app.pairing.StoredMailbox

/**
 * "Only same Wi-Fi or my own server" (`alpha-connection-model` design section 8). With the switch
 * on this device contacts no project server: its invites and stored set leave out the project
 * entries, a project entry of a received invite or mailbox update is ignored, and a socket to the
 * project host is never opened. The core then sees no project relay, so it asks for no project STUN
 * or TURN either. The switch acts on this device only; the platform just removes entries, it never
 * picks another address (the mailbox set comes from the pairing record alone).
 */
object OnlyLocal {
    fun isProjectUrl(url: String, projectHost: String = PublicLists.projectServer.host): Boolean {
        val host = runCatching { URI(url.trim()).host }.getOrNull() ?: return false
        return host.equals(projectHost, ignoreCase = true)
    }

    /** [urls] without the project server when [onlyLocal]; unchanged otherwise. */
    fun urls(urls: List<String>, onlyLocal: Boolean): List<String> =
        if (onlyLocal) urls.filterNot { isProjectUrl(it) } else urls

    fun mailboxes(set: List<StoredMailbox>, onlyLocal: Boolean): List<StoredMailbox> =
        if (onlyLocal) set.filterNot { isProjectUrl(it.url) } else set
}
