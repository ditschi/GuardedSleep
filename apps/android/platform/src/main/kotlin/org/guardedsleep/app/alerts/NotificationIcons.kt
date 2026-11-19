package org.guardedsleep.app.alerts

import androidx.annotation.DrawableRes
import org.guardedsleep.app.platform.R
import org.guardedsleep.app.ui.model.Role

/**
 * The small icon of every notification, by the role the device runs as: the crescent for the baby
 * device, the crescent with listening bars for the parent device (`adopt-night-crescent-logo`). A
 * notification with no role yet gets the baby crescent, the plain brand mark.
 */
object NotificationIcons {
    @DrawableRes
    fun forRole(role: Role?): Int =
        if (role == Role.PARENT) R.drawable.ic_stat_parent else R.drawable.ic_stat_baby
}
