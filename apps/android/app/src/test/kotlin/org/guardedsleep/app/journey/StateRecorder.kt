package org.guardedsleep.app.journey

import java.lang.reflect.Field
import java.lang.reflect.Modifier

/**
 * Collects which leaf fields of the UI state models ever left their default during the journeys
 * (`ui-journey-tests` design, "Behavioural writer check"). A source scan cannot see a field that is
 * declared and read but never written to a real value; sampling the state the user would see can.
 *
 * A leaf is a field of a model data class, nested model classes included. "Default" is the value in
 * a default-constructed instance when the class has one, else the type's zero value (null, 0,
 * false, empty, first enum constant). A list leaf is non-default once it holds an element.
 */
object StateRecorder {
    private val seen = mutableSetOf<String>()

    @Synchronized
    fun sample(state: Any?) {
        if (state == null) return
        val cls = state.javaClass
        walk(state, defaultInstance(cls), cls.simpleName)
    }

    @Synchronized fun seenPaths(): Set<String> = seen.toSet()

    @Synchronized
    fun allPaths(roots: List<Class<*>>): Set<String> =
        roots.flatMap { leafPaths(it, it.simpleName) }.toSet()

    /** The leaf paths of [roots] that no journey wrote to a non-default value. */
    @Synchronized
    fun unseen(roots: List<Class<*>>): List<String> {
        val all = roots.flatMap { leafPaths(it, it.simpleName) }.sorted()
        return all.filterNot { it in seen }
    }

    private fun isModel(cls: Class<*>) =
        cls.name.startsWith("org.guardedsleep.app.ui.model.") &&
            cls.declaredMethods.any { it.name == "copy" }

    private fun fields(cls: Class<*>): List<Field> =
        cls.declaredFields
            .filter { !Modifier.isStatic(it.modifiers) && !it.isSynthetic }
            .onEach { it.isAccessible = true }

    private fun leafPaths(cls: Class<*>, prefix: String): List<String> =
        fields(cls).flatMap {
            if (isModel(it.type)) leafPaths(it.type, "$prefix.${it.name}")
            else listOf("$prefix.${it.name}")
        }

    private fun defaultInstance(cls: Class<*>): Any? = runCatching {
        cls.getDeclaredConstructor().newInstance()
    }
        .getOrNull()

    private fun walk(obj: Any, base: Any?, prefix: String) {
        for (f in fields(obj.javaClass)) {
            val value = f.get(obj)
            val baseValue = base?.let { f.get(it) }
            val path = "$prefix.${f.name}"
            // Only a field declared as a model class is nested; a sealed type (TalkUi) is one leaf.
            if (value != null && isModel(f.type)) {
                walk(value, baseValue ?: defaultInstance(value.javaClass), path)
            } else if (differsFromDefault(value, baseValue, base != null)) {
                seen += path
            }
        }
    }

    private fun differsFromDefault(value: Any?, base: Any?, hasBase: Boolean): Boolean =
        if (hasBase) value != base
        else
            when (value) {
                null -> false
                is Boolean -> value
                is Number -> value.toDouble() != 0.0
                is CharSequence -> value.isNotEmpty()
                is Collection<*> -> value.isNotEmpty()
                is Map<*, *> -> value.isNotEmpty()
                is Enum<*> -> value.ordinal != 0
                else -> true
            }

    // ---- bookkeeping for the final check ----------------------------------------------------

    private var passed = 0

    @Synchronized
    fun journeyPassed() {
        passed++
    }

    @Synchronized fun journeysPassed() = passed
}
