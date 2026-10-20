package org.guardedsleep.app;

import android.util.Log;
import com.getcapacitor.PluginCall;
import java.net.URI;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.Response;
import okhttp3.WebSocket;
import okhttp3.WebSocketListener;
import okio.ByteString;

/** OkHttp WebSocket transport for signaling (dev self-signed WSS on LAN). */
final class NativeSignalingWebSocket extends WebSocketListener {

  private static final String TAG = "NativeSignalingWs";

  private final GuardedSleepNativePlugin plugin;
  private final PluginCall connectCall;
  private final OkHttpClient client;
  private WebSocket webSocket;
  private boolean connectSettled;

  NativeSignalingWebSocket(GuardedSleepNativePlugin plugin, String url, PluginCall connectCall) {
    this.plugin = plugin;
    this.connectCall = connectCall;
    String host;
    try {
      host = URI.create(url).getHost();
    } catch (Exception e) {
      host = "";
    }
    OkHttpClient.Builder builder = new OkHttpClient.Builder();
    this.client = DevWssTrust.configureClient(builder, host).build();
    Request request = new Request.Builder().url(url).build();
    this.webSocket = this.client.newWebSocket(request, this);
  }

  void send(String text) {
    WebSocket ws = webSocket;
    if (ws != null) {
      ws.send(text);
    }
  }

  void closeQuietly() {
    WebSocket ws = webSocket;
    webSocket = null;
    if (ws != null) {
      try {
        ws.close(1000, "client close");
      } catch (Exception e) {
        Log.w(TAG, "close", e);
      }
    }
    client.dispatcher().executorService().shutdown();
  }

  private void settleConnect(boolean ok, String message) {
    if (connectSettled) {
      return;
    }
    connectSettled = true;
    if (ok) {
      connectCall.resolve();
    } else {
      connectCall.reject(message != null ? message : "connect failed");
    }
  }

  @Override
  public void onOpen(WebSocket webSocket, Response response) {
    settleConnect(true, null);
  }

  @Override
  public void onMessage(WebSocket webSocket, String text) {
    plugin.emitNativeSignalingMessage(text);
  }

  @Override
  public void onMessage(WebSocket webSocket, ByteString bytes) {
    onMessage(webSocket, bytes.utf8());
  }

  @Override
  public void onClosing(WebSocket webSocket, int code, String reason) {
    webSocket.close(code, reason);
  }

  @Override
  public void onClosed(WebSocket webSocket, int code, String reason) {
    plugin.emitNativeSignalingClosed();
    if (!connectSettled) {
      settleConnect(false, "closed before open");
    }
  }

  @Override
  public void onFailure(WebSocket webSocket, Throwable t, Response response) {
    Log.w(TAG, "failure", t);
    plugin.emitNativeSignalingClosed();
    settleConnect(false, t != null ? t.getMessage() : "failure");
  }
}
