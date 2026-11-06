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

    /**
     * The socket failed or the server closed it. Not called after our own
     * [SignalingTransport.close].
     */
    fun onClosed()
}

/** One WebSocket to the signaling server; all protocol decisions live in the Rust core. */
interface SignalingTransport {
    fun open()

    fun send(text: String): Boolean

    fun close()
}

fun interface SignalingFactory {
    fun create(url: String, listener: SignalingListener): SignalingTransport
}

class OkHttpSignalingFactory(
    private val client: OkHttpClient =
        OkHttpClient.Builder()
            .connectTimeout(8, TimeUnit.SECONDS)
            .readTimeout(0, TimeUnit.SECONDS)
            .build()
) : SignalingFactory {
    override fun create(url: String, listener: SignalingListener): SignalingTransport =
        OkHttpSignalingTransport(client, url, listener)
}

/**
 * One WebSocket to exactly the address the core named in `open_mailbox`. A failure before it opened
 * is reported once as closed; the core applies its backoff and decides whether to open this or
 * another relay mailbox. The transport never retries or tries another address.
 */
class OkHttpSignalingTransport(
    private val client: OkHttpClient,
    private val url: String,
    private val listener: SignalingListener,
) : SignalingTransport {
    @Volatile private var socket: WebSocket? = null
    @Volatile private var closedByUs = false
    private val lock = Any()
    private var generation = 0

    override fun open() {
        connect(synchronized(lock) { ++generation })
    }

    override fun send(text: String): Boolean = socket?.send(text) ?: false

    override fun close() {
        closedByUs = true
        synchronized(lock) { generation++ }
        socket?.close(1000, null)
        socket = null
    }

    private fun connect(gen: Int) {
        if (closedByUs) return
        val request = runCatching { Request.Builder().url(url).build() }.getOrNull()
        if (request == null) return ended(gen)
        socket =
            client.newWebSocket(
                request,
                object : WebSocketListener() {
                    override fun onOpen(webSocket: WebSocket, response: Response) {
                        if (!current(gen)) return webSocket.cancel()
                        listener.onOpened()
                    }

                    override fun onMessage(webSocket: WebSocket, text: String) {
                        if (current(gen)) listener.onMessage(text)
                    }

                    override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                        webSocket.close(1000, null)
                    }

                    override fun onClosed(webSocket: WebSocket, code: Int, reason: String) =
                        ended(gen)

                    override fun onFailure(
                        webSocket: WebSocket,
                        t: Throwable,
                        response: Response?,
                    ) = ended(gen)
                },
            )
    }

    private fun ended(gen: Int) {
        if (current(gen)) listener.onClosed()
    }

    private fun current(gen: Int) = !closedByUs && synchronized(lock) { generation == gen }
}
