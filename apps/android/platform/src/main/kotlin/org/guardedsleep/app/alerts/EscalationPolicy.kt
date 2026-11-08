package org.guardedsleep.app.alerts

import org.guardedsleep.app.ui.model.AlertStyle
import org.guardedsleep.app.ui.model.DiscreetEscalation

/**
 * What the platform hands to the core's noise escalation timer. The core knows nothing about the
 * alert style: it gets a delay only while the style is discreet and the escalation is not off (null
 * switches its timer off). The timer itself runs in the core, never here.
 */
object EscalationPolicy {
    fun delayMs(style: AlertStyle, escalation: DiscreetEscalation): Long? {
        if (style != AlertStyle.DISCREET) return null
        return escalation.minutes?.let { it * 60_000L }
    }
}
