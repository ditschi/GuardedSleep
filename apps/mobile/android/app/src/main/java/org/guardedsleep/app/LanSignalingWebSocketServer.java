package org.guardedsleep.app;

import java.net.InetSocketAddress;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.Executors;
import java.util.concurrent.ScheduledExecutorService;
import java.util.concurrent.ScheduledFuture;
import java.util.concurrent.TimeUnit;
import org.java_websocket.WebSocket;
import org.java_websocket.handshake.ClientHandshake;
import org.java_websocket.server.WebSocketServer;
import org.json.JSONArray;
import org.json.JSONObject;

/**
 * Minimal in-process signaling relay matching {@code apps/signaling-server/src/server.ts} +
 * {@code packages/protocol} (hello / signal / ping). Single {@code boundRoomId} per instance
 * (baby unit); clients must send {@code hello.roomId} equal to that value.
 */
public final class LanSignalingWebSocketServer extends WebSocketServer {

  private static final int PROTOCOL_VERSION = 2;
  private static final int MAX_PARENTS = 32;
  private static final int HELLO_TIMEOUT_MS = 30_000;

  private final String boundRoomId;
  private final ScheduledExecutorService scheduler = Executors.newSingleThreadScheduledExecutor();
  private final SignalingRoom room;

  private static final class SignalingRoom {
    private final Map<String, WebSocket> peers = new HashMap<>();
    private final Map<String, String> roles = new HashMap<>();

    synchronized String addPeer(String id, String role, WebSocket socket) {
      if ("baby".equals(role)) {
        for (String r : roles.values()) {
          if ("baby".equals(r)) return "baby_taken";
        }
      } else {
        int parents = 0;
        for (String r : roles.values()) {
          if ("parent".equals(r)) parents++;
        }
        if (parents >= MAX_PARENTS) return "parent_cap";
      }
      peers.put(id, socket);
      roles.put(id, role);
      return "ok";
    }

    synchronized void remove(String id) {
      peers.remove(id);
      roles.remove(id);
    }

    synchronized WebSocket getOther(String exceptId) {
      for (Map.Entry<String, WebSocket> e : peers.entrySet()) {
        if (!e.getKey().equals(exceptId)) return e.getValue();
      }
      return null;
    }

    synchronized WebSocket getById(String id) {
      return peers.get(id);
    }

    synchronized int size() {
      return peers.size();
    }

    synchronized JSONArray listMembers() {
      JSONArray arr = new JSONArray();
      for (Map.Entry<String, String> e : roles.entrySet()) {
        JSONObject m = new JSONObject();
        m.put("clientId", e.getKey());
        m.put("role", e.getValue());
        arr.put(m);
      }
      return arr;
    }
  }

  private static final class ClientState {
    volatile String joinedRoomId;
    volatile String clientId;
    volatile ScheduledFuture<?> helloTimer;
  }

  private final Map<WebSocket, ClientState> states = new ConcurrentHashMap<>();

  public LanSignalingWebSocketServer(String boundRoomId, int port) {
    super(new InetSocketAddress("0.0.0.0", port));
    this.boundRoomId = boundRoomId;
    this.room = new SignalingRoom();
    setReuseAddr(true);
  }

  @Override
  public void onOpen(WebSocket conn, ClientHandshake handshake) {
    ClientState st = new ClientState();
    states.put(conn, st);
    st.helloTimer =
        scheduler.schedule(
            () -> {
              if (st.joinedRoomId == null && conn.isOpen()) {
                conn.close(1008, "hello timeout");
              }
            },
            HELLO_TIMEOUT_MS,
            TimeUnit.MILLISECONDS);
  }

  @Override
  public void onClose(WebSocket conn, int code, String reason, boolean remote) {
    ClientState st = states.remove(conn);
    if (st == null) return;
    if (st.helloTimer != null) {
      st.helloTimer.cancel(false);
    }
    if (st.clientId != null) {
      room.remove(st.clientId);
      broadcastMembers();
    }
  }

  @Override
  public void onMessage(WebSocket conn, String message) {
    ClientState st = states.get(conn);
    if (st == null) return;

    JSONObject msg;
    try {
      msg = new JSONObject(message);
    } catch (Exception e) {
      sendJson(conn, err("BAD_JSON", "invalid message"));
      return;
    }
    String type = msg.optString("type", "");

    if ("ping".equals(type)) {
      sendJson(conn, new JSONObject().put("type", "pong"));
      return;
    }

    if ("hello".equals(type)) {
      if (st.joinedRoomId != null) {
        sendJson(conn, err("INVALID_MESSAGE", "already joined"));
        return;
      }
      String role = msg.optString("role", "");
      String roomId = msg.optString("roomId", "");
      int pv = msg.optInt("protocolVersion", -1);
      if (!"baby".equals(role) && !"parent".equals(role)) {
        sendJson(conn, err("INVALID_MESSAGE", "invalid role"));
        return;
      }
      if (roomId.isEmpty() || roomId.length() > 128) {
        sendJson(conn, err("INVALID_MESSAGE", "invalid roomId"));
        return;
      }
      if (pv != PROTOCOL_VERSION) {
        sendJson(conn, err("UNSUPPORTED_PROTOCOL", "version mismatch"));
        conn.close(1008, "version mismatch");
        return;
      }
      if (!boundRoomId.equals(roomId)) {
        sendJson(conn, err("INVALID_MESSAGE", "room mismatch"));
        conn.close(1008, "room mismatch");
        return;
      }

      String clientId = UUID.randomUUID().toString();
      String status = room.addPeer(clientId, role, conn);
      if ("baby_taken".equals(status)) {
        sendJson(conn, err("BABY_TAKEN", "room already has a baby unit"));
        conn.close(1008, "baby taken");
        return;
      }
      if ("parent_cap".equals(status)) {
        sendJson(conn, err("PARENT_CAP", "too many parent units"));
        conn.close(1008, "parent cap");
        return;
      }

      st.joinedRoomId = roomId;
      st.clientId = clientId;
      if (st.helloTimer != null) {
        st.helloTimer.cancel(false);
        st.helloTimer = null;
      }

      sendJson(conn, new JSONObject().put("type", "welcome").put("clientId", clientId).put("roomId", roomId));
      broadcastMembers();

      for (WebSocket w : listOpenSockets()) {
        ClientState o = states.get(w);
        if (o != null && o.clientId != null && !o.clientId.equals(clientId)) {
          sendJson(w, new JSONObject().put("type", "peer").put("peerId", clientId));
        }
      }
      WebSocket other = room.getOther(clientId);
      String otherId = other != null ? idForSocket(other) : null;
      JSONObject peerMsg = new JSONObject().put("type", "peer");
      if (otherId == null) {
        peerMsg.put("peerId", JSONObject.NULL);
      } else {
        peerMsg.put("peerId", otherId);
      }
      sendJson(conn, peerMsg);
      return;
    }

    if (st.joinedRoomId == null) {
      sendJson(conn, err("NOT_JOINED", "send hello first"));
      return;
    }

    if ("signal".equals(type)) {
      String roomId = msg.optString("roomId", "");
      if (!st.joinedRoomId.equals(roomId)) {
        sendJson(conn, err("INVALID_MESSAGE", "room mismatch"));
        return;
      }
      if (!msg.has("payload")) {
        sendJson(conn, err("INVALID_MESSAGE", "missing payload"));
        return;
      }
      String to = msg.has("to") && !msg.isNull("to") ? msg.optString("to", "") : "";
      WebSocket target;
      if (to != null && !to.isEmpty()) {
        target = room.getById(to);
        if (target == null || target == conn) {
          sendJson(conn, err("INVALID_MESSAGE", "unknown signal target"));
          return;
        }
      } else if (room.size() <= 2) {
        target = room.getOther(st.clientId);
      } else {
        sendJson(
            conn,
            err("SIGNAL_TARGET_REQUIRED", "set signal.to when more than two peers are in the room"));
        return;
      }
      if (target == null) return;
      sendJson(
          target,
          new JSONObject().put("type", "signal").put("from", st.clientId).put("payload", msg.get("payload")));
    }
  }

  private String idForSocket(WebSocket w) {
    for (Map.Entry<WebSocket, ClientState> e : states.entrySet()) {
      if (e.getKey() == w && e.getValue().clientId != null) return e.getValue().clientId;
    }
    return null;
  }

  private List<WebSocket> listOpenSockets() {
    List<WebSocket> out = new ArrayList<>();
    for (WebSocket w : states.keySet()) {
      if (w.isOpen()) out.add(w);
    }
    return out;
  }

  private void broadcastMembers() {
    JSONObject m = new JSONObject().put("type", "members").put("peers", room.listMembers());
    String raw = m.toString();
    for (WebSocket w : listOpenSockets()) {
      ClientState o = states.get(w);
      if (o != null && o.clientId != null) w.send(raw);
    }
  }

  private static JSONObject err(String code, String message) {
    return new JSONObject().put("type", "error").put("code", code).put("message", message);
  }

  private static void sendJson(WebSocket w, JSONObject o) {
    if (w.isOpen()) w.send(o.toString());
  }

  @Override
  public void onError(WebSocket conn, Exception ex) {
    /* ignore — client disconnects are common */
  }

  @Override
  public void onStart() {
    /* no-op */
  }

  @Override
  public void stop() throws InterruptedException {
    scheduler.shutdownNow();
    super.stop();
  }
}
