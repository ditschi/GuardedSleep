package org.guardedsleep.app.fixtures

import java.util.PriorityQueue
import java.util.concurrent.AbstractExecutorService
import java.util.concurrent.Callable
import java.util.concurrent.Delayed
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.ScheduledExecutorService
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit
import org.guardedsleep.app.link.Clock

/**
 * Virtual time for the journey tests: one queue and one clock for everything that would otherwise
 * run on a thread or sleep (the core, the relay fake, the TURN and device-check writers). Nothing
 * runs until the test thread calls [advanceBy] or [runUntilIdle], so a run is deterministic and a
 * 60 s link loss costs no wall time.
 *
 * [executor] hands out one [ScheduledExecutorService] view per owner (each `LinkRunner` shuts its
 * own down at teardown without touching the others); all views share this queue and clock.
 * Exceptions thrown by a task are collected and rethrown as an [AssertionError] when the step ends
 * (a thread pool would only print them).
 */
class ManualScheduler(startElapsedMs: Long = 1_000, startWallMs: Long = 1_700_000_000_000) : Clock {
    private var nowMs = startElapsedMs
    private val wallOffsetMs = startWallMs - startElapsedMs
    private var seq = 0L
    private val queue = PriorityQueue<Job>(compareBy<Job>({ it.dueMs }, { it.seq }))
    private val failures = mutableListOf<Throwable>()

    override fun elapsedMs() = nowMs

    override fun wallMs() = nowMs + wallOffsetMs

    /** A fresh executor view; shutting it down cancels its periodic jobs only. */
    fun executor(): ScheduledExecutorService = View()

    /** Runs [block] at the current time, after the tasks already due. */
    fun post(block: () -> Unit) {
        enqueue(0, null, null, null, block)
    }

    /** Runs everything due now (and whatever it adds for now), not the future. */
    fun runUntilIdle() {
        drain(nowMs)
    }

    /** Moves time forward by [ms], running every job in due order on the way. */
    fun advanceBy(ms: Long) {
        require(ms >= 0)
        val target = nowMs + ms
        drain(target)
        nowMs = target
        rethrow()
    }

    /** Steps by [stepMs] until [cond] holds, at most [timeoutMs] of virtual time. */
    fun runUntil(what: String, timeoutMs: Long = 15_000, stepMs: Long = 25, cond: () -> Boolean) {
        runUntilIdle()
        rethrow()
        var waited = 0L
        while (!cond()) {
            if (waited >= timeoutMs) throw AssertionError("timeout waiting for $what")
            advanceBy(stepMs)
            waited += stepMs
        }
    }

    private fun drain(untilMs: Long) {
        var guard = 0
        while (true) {
            val job = queue.peek() ?: break
            if (job.dueMs > untilMs) break
            queue.poll()
            if (job.cancelled) continue
            if (job.dueMs > nowMs) nowMs = job.dueMs
            check(++guard < MAX_JOBS_PER_STEP) { "runaway task loop in the scheduler" }
            try {
                job.run()
            } catch (t: Throwable) {
                failures += t
            }
            if (job.periodMs != null && !job.cancelled && !job.owner.isShutdown()) {
                job.dueMs = nowMs + job.periodMs
                job.seq = seq++
                queue.add(job)
            }
        }
    }

    private fun rethrow() {
        if (failures.isEmpty()) return
        val first = failures.first()
        failures.clear()
        throw AssertionError("a scheduled task failed: $first", first)
    }

    private fun enqueue(
        delayMs: Long,
        periodMs: Long?,
        owner: View?,
        callable: Callable<Any?>?,
        runnable: (() -> Unit)?,
    ): Job {
        val job =
            Job(
                nowMs + delayMs.coerceAtLeast(0),
                seq++,
                periodMs,
                owner ?: View(),
                callable,
                runnable,
            )
        queue.add(job)
        return job
    }

    private inner class Job(
        var dueMs: Long,
        var seq: Long,
        val periodMs: Long?,
        val owner: View,
        private val callable: Callable<Any?>?,
        private val runnable: (() -> Unit)?,
    ) : ScheduledFuture<Any?> {
        @Volatile var cancelled = false
        private var done = false
        private var result: Any? = null

        fun run() {
            result = callable?.call() ?: runnable?.invoke()
            if (periodMs == null) done = true
        }

        override fun getDelay(unit: TimeUnit) = unit.convert(dueMs - nowMs, TimeUnit.MILLISECONDS)

        override fun compareTo(other: Delayed) =
            getDelay(TimeUnit.MILLISECONDS).compareTo(other.getDelay(TimeUnit.MILLISECONDS))

        override fun cancel(mayInterruptIfRunning: Boolean): Boolean {
            val was = cancelled || done
            cancelled = true
            return !was
        }

        override fun isCancelled() = cancelled

        override fun isDone() = cancelled || done

        // Nothing blocks on a result in the tests: a value exists once the job ran.
        override fun get(): Any? {
            check(done) { "the job has not run; advance the scheduler" }
            return result
        }

        override fun get(timeout: Long, unit: TimeUnit) = get()
    }

    private inner class View : AbstractExecutorService(), ScheduledExecutorService {
        @Volatile private var down = false

        private fun reject() {
            if (down) throw RejectedExecutionException("executor view is shut down")
        }

        override fun execute(command: Runnable) {
            reject()
            enqueue(0, null, this, null) { command.run() }
        }

        override fun schedule(command: Runnable, delay: Long, unit: TimeUnit): ScheduledFuture<*> {
            reject()
            return enqueue(unit.toMillis(delay), null, this, null) { command.run() }
        }

        override fun <V : Any?> schedule(
            callable: Callable<V>,
            delay: Long,
            unit: TimeUnit,
        ): ScheduledFuture<V> {
            reject()
            @Suppress("UNCHECKED_CAST")
            return enqueue(unit.toMillis(delay), null, this, callable as Callable<Any?>, null)
                as ScheduledFuture<V>
        }

        override fun scheduleAtFixedRate(
            command: Runnable,
            initialDelay: Long,
            period: Long,
            unit: TimeUnit,
        ): ScheduledFuture<*> = scheduleWithFixedDelay(command, initialDelay, period, unit)

        override fun scheduleWithFixedDelay(
            command: Runnable,
            initialDelay: Long,
            delay: Long,
            unit: TimeUnit,
        ): ScheduledFuture<*> {
            reject()
            return enqueue(unit.toMillis(initialDelay), unit.toMillis(delay), this, null) {
                command.run()
            }
        }

        override fun shutdown() {
            down = true
        }

        override fun shutdownNow(): MutableList<Runnable> {
            down = true
            queue.filter { it.owner === this }.forEach { it.cancel(false) }
            return mutableListOf()
        }

        override fun isShutdown() = down

        override fun isTerminated() = down && queue.none { it.owner === this && !it.isDone }

        override fun awaitTermination(timeout: Long, unit: TimeUnit) = isTerminated
    }

    private companion object {
        const val MAX_JOBS_PER_STEP = 1_000_000
    }
}
