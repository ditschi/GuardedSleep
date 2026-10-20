package org.guardedsleep.app.signaling

import java.util.concurrent.TimeUnit
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener

/** What the transport reports; callbacks arrive on network threads. */
interface SignalingListener {
    fun onOpened()

    fun onMessage(text: String)

    /** The socket failed or the server closed it. Not called after our own [SignalingTransport.close]. */
    fun onClosed()
}

/** One WebSocket to the signaling server; all protocol decisions live in the Rust core. */
interface SignalingTransport {
    fun open()

    fun send(text: String): Boolean

    fun close()
}

fun interface SignalingFactory {
    fun create(urls: List<String>, listener: SignalingListener): SignalingTransport
}

class OkHttpSignalingFactory(
    private val client: OkHttpClient =
        OkHttpClient.Builder().connectTimeout(8, TimeUnit.SECONDS).readTimeout(0, TimeUnit.SECONDS).build(),
) : SignalingFactory {
    override fun create(urls: List<String>, listener: SignalingListener): SignalingTransport =
        OkHttpSignalingTransport(client, urls, listener)
}

/**
 * Tries the configured URLs in order each time the core opens signaling (the first is the
 * primary, so a recovered primary is preferred again). A URL that fails before it opened moves on
 * to the next; when all fail, the core is told once and applies its own backoff.
 */
class OkHttpSignalingTransport(
    private val client: OkHttpClient,
    private val urls: List<String>,
    private val listener: SignalingListener,
) : SignalingTransport {
    @Volatile private var socket: WebSocket? = null
    @Volatile private var closedByUs = false
    private val lock = Any()
    private var generation = 0

    override fun open() {
        connect(0, synchronized(lock) { ++generation })
    }

    override fun send(text: String): Boolean = socket?.send(text) ?: false

    override fun close() {
        closedByUs = true
        synchronized(lock) { generation++ }
        socket?.close(1000, null)
        socket = null
    }

    private fun connect(index: Int, gen: Int) {
        if (closedByUs || index >= urls.size) return
        val request = runCatching { Request.Builder().url(urls[index]).build() }.getOrNull()
        if (request == null) return advance(index, gen)
        var opened = false
        socket =
            client.newWebSocket(
                request,
                object : WebSocketListener() {
                    override fun onOpen(webSocket: WebSocket, response: Response) {
                        if (!current(gen)) return webSocket.cancel()
                        opened = true
                        listener.onOpened()
                    }

                    override fun onMessage(webSocket: WebSocket, text: String) {
                        if (current(gen)) listener.onMessage(text)
                    }

                    override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                        webSocket.close(1000, null)
                    }

                    override fun onClosed(webSocket: WebSocket, code: Int, reason: String) = ended()

                    override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) = ended()

                    private fun ended() {
                        if (!current(gen)) return
                        if (opened) listener.onClosed() else advance(index, gen)
                    }
                },
            )
    }

    private fun advance(index: Int, gen: Int) {
        if (!current(gen)) return
        if (index + 1 < urls.size) connect(index + 1, gen) else listener.onClosed()
    }

    private fun current(gen: Int) = !closedByUs && synchronized(lock) { generation == gen }
}
