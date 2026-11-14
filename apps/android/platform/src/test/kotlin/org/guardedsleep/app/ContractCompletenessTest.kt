package org.guardedsleep.app

import org.guardedsleep.app.ui.model.AppController
import org.junit.Assert.assertEquals
import org.junit.Test

/**
 * `ui-journey-tests`, "Complete UI contract": the Kotlin `AppController` declares no default method
 * bodies, so a new method cannot compile into `LinkController`, `FakeAppController` or a preview
 * without an implementation. A default body shows up either as a default interface method or, with
 * the Kotlin default settings, as a `DefaultImpls` class.
 */
class ContractCompletenessTest {
    @Test
    fun appControllerHasNoDefaultMethodBodies() {
        val cls = AppController::class.java
        val defaults = cls.declaredMethods.filter { it.isDefault }.map { it.name }.sorted()
        assertEquals("methods with a default body in AppController", emptyList<String>(), defaults)
        // Default arguments (`chooseRole(role, makeDefault = false)`) also compile into a
        // DefaultImpls class, as `name$default` helpers; only other members are method bodies.
        val bodies =
            cls.declaredClasses
                .filter { it.simpleName == "DefaultImpls" }
                .flatMap { it.declaredMethods.map { m -> m.name } }
                .filterNot { it.endsWith("\$default") }
                .sorted()
        assertEquals(
            "methods with a body in AppController.DefaultImpls",
            emptyList<String>(),
            bodies,
        )
    }
}
