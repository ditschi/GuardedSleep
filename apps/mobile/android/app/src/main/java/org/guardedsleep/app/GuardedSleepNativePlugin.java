package org.guardedsleep.app;

import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.content.Context;
import android.content.Intent;
import android.net.nsd.NsdManager;
import android.net.nsd.NsdServiceInfo;
import android.net.wifi.WifiManager;
import android.app.Activity;
import android.os.Build;
import android.provider.Settings;
import android.util.Log;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.WindowManager;
import androidx.core.app.NotificationCompat;
import com.getcapacitor.JSObject;
import com.getcapacitor.Plugin;
import com.getcapacitor.PluginCall;
import com.getcapacitor.PluginMethod;
import com.getcapacitor.annotation.CapacitorPlugin;
import java.util.UUID;

@CapacitorPlugin(name = "GuardedSleepNative")
public class GuardedSleepNativePlugin extends Plugin {

  private static final String TAG = "GuardedSleepNative";
  private static final String SERVICE_TYPE = "_guardedsleep._tcp.";
  private static final String PARENT_ALARM_CHANNEL_ID = "guardedsleep_parent_alarm";

  private NsdManager nsdManager;
  private NsdManager.RegistrationListener registrationListener;
  private NsdManager.DiscoveryListener discoveryListener;
  private boolean advertising = false;
  private boolean discovering = false;
  /** Last immersive mode requested from JS (re-applied on resume, e.g. after returning from settings). */
  private boolean immersiveModeWanted = false;

  private LanSignalingWebSocketServer lanSignalingServer;
  private WifiManager.WifiLock wifiLock;
  private WifiManager.MulticastLock multicastLock;

  @PluginMethod
  public void bringToForeground(PluginCall call) {
    android.app.Activity a = getActivity();
    if (a == null) {
      call.reject("No activity");
      return;
    }
    a.runOnUiThread(
        () -> {
          android.content.Intent i = new android.content.Intent(a, a.getClass());
          i.setFlags(
              android.content.Intent.FLAG_ACTIVITY_REORDER_TO_FRONT
                  | android.content.Intent.FLAG_ACTIVITY_SINGLE_TOP);
          a.startActivity(i);
        });
    call.resolve();
  }

  private void ensureParentAlarmChannel(Context ctx) {
    if (Build.VERSION.SDK_INT < Build.VERSION_CODES.O) {
      return;
    }
    NotificationManager nm = (NotificationManager) ctx.getSystemService(Context.NOTIFICATION_SERVICE);
    if (nm == null) {
      return;
    }
    NotificationChannel ch =
        new NotificationChannel(
            PARENT_ALARM_CHANNEL_ID,
            "Baby monitor alerts",
            NotificationManager.IMPORTANCE_HIGH);
    ch.setDescription("Noise and connection alerts (alarm-style)");
    ch.enableVibration(true);
    ch.setLockscreenVisibility(NotificationCompat.VISIBILITY_PUBLIC);
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
      ch.setBypassDnd(true);
    }
    nm.createNotificationChannel(ch);
  }

  /**
   * Alarm-category notification with full-screen intent (where the OS allows it) so the parent
   * unit can wake the screen like a classic baby monitor.
   */
  @PluginMethod
  public void showAlarmStyleNotification(PluginCall call) {
    Context ctx = getContext();
    if (ctx == null) {
      call.reject("No context");
      return;
    }
    String title = call.getString("title", "GuardedSleep");
    String body = call.getString("body", "");
    int notificationId = call.getInt("notificationId", 71001);

    NotificationManager nm = (NotificationManager) ctx.getSystemService(Context.NOTIFICATION_SERVICE);
    if (nm == null) {
      call.reject("No NotificationManager");
      return;
    }

    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O) {
      ensureParentAlarmChannel(ctx);
    }

    Intent launch = new Intent(ctx, MainActivity.class);
    launch.setFlags(
        Intent.FLAG_ACTIVITY_NEW_TASK
            | Intent.FLAG_ACTIVITY_SINGLE_TOP
            | Intent.FLAG_ACTIVITY_CLEAR_TOP);

    PendingIntent fullScreenPi =
        PendingIntent.getActivity(
            ctx,
            notificationId,
            launch,
            PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);

    int smallIcon = R.drawable.ic_launcher_foreground;

    NotificationCompat.Builder b =
        new NotificationCompat.Builder(ctx, PARENT_ALARM_CHANNEL_ID)
            .setSmallIcon(smallIcon)
            .setContentTitle(title)
            .setContentText(body)
            .setStyle(new NotificationCompat.BigTextStyle().bigText(body))
            .setPriority(NotificationCompat.PRIORITY_MAX)
            .setCategory(NotificationCompat.CATEGORY_ALARM)
            .setVisibility(NotificationCompat.VISIBILITY_PUBLIC)
            .setAutoCancel(true)
            .setContentIntent(fullScreenPi)
            .setFullScreenIntent(fullScreenPi, true);

    nm.notify(notificationId, b.build());
    call.resolve();
  }

  @PluginMethod
  public void openBatterySettings(PluginCall call) {
    Activity activity = getActivity();
    Context ctx = getContext();
    if (activity == null && ctx == null) {
      call.reject("No context");
      return;
    }
    String pkg = activity != null ? activity.getPackageName() : ctx.getPackageName();
    Intent intent = new Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS);
    intent.setData(android.net.Uri.parse("package:" + pkg));
    try {
      if (activity != null) {
        activity.startActivity(intent);
      } else {
        intent.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK);
        ctx.startActivity(intent);
      }
      call.resolve();
    } catch (Exception e) {
      Log.e(TAG, "openBatterySettings", e);
      call.reject("Could not open app settings", e);
    }
  }

  @PluginMethod
  public void setKeepAwake(PluginCall call) {
    Boolean on = call.getBoolean("enabled", false);
    if (getActivity() == null) {
      call.reject("No activity");
      return;
    }
    getActivity()
        .runOnUiThread(
            () -> {
              if (on) {
                getActivity().getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
              } else {
                getActivity().getWindow().clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
              }
            });
    call.resolve();
  }

  @PluginMethod
  public void setImmersiveMode(PluginCall call) {
    immersiveModeWanted = call.getBoolean("enabled", false);
    Activity activity = getActivity();
    if (activity == null) {
      call.reject("No activity");
      return;
    }
    activity.runOnUiThread(() -> applyImmersiveToActivity(activity, immersiveModeWanted));
    call.resolve();
  }

  private void applyImmersiveToActivity(Activity activity, boolean on) {
    View decor = activity.getWindow().getDecorView();
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.R) {
      WindowInsetsController controller = activity.getWindow().getInsetsController();
      if (controller != null) {
        if (on) {
          controller.hide(WindowInsets.Type.statusBars() | WindowInsets.Type.navigationBars());
          controller.setSystemBarsBehavior(
              WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
        } else {
          controller.show(WindowInsets.Type.statusBars() | WindowInsets.Type.navigationBars());
        }
      }
      return;
    }
    if (on) {
      int flags =
          View.SYSTEM_UI_FLAG_IMMERSIVE_STICKY
              | View.SYSTEM_UI_FLAG_FULLSCREEN
              | View.SYSTEM_UI_FLAG_HIDE_NAVIGATION
              | View.SYSTEM_UI_FLAG_LAYOUT_STABLE
              | View.SYSTEM_UI_FLAG_LAYOUT_HIDE_NAVIGATION
              | View.SYSTEM_UI_FLAG_LAYOUT_FULLSCREEN;
      decor.setSystemUiVisibility(flags);
      decor.setOnSystemUiVisibilityChangeListener(
          visibility -> {
            if ((visibility & View.SYSTEM_UI_FLAG_FULLSCREEN) == 0) {
              decor.setSystemUiVisibility(flags);
            }
          });
    } else {
      decor.setOnSystemUiVisibilityChangeListener(null);
      decor.setSystemUiVisibility(View.SYSTEM_UI_FLAG_VISIBLE);
    }
  }

  @Override
  protected void handleOnResume() {
    super.handleOnResume();
    Activity activity = getActivity();
    if (activity != null && immersiveModeWanted) {
      activity.runOnUiThread(() -> applyImmersiveToActivity(activity, true));
    }
  }

  @PluginMethod
  public void startLanSignalingServer(PluginCall call) {
    String roomId = call.getString("roomId", "");
    if (roomId.isEmpty()) {
      call.reject("roomId required");
      return;
    }
    Context ctx = getContext();
    if (ctx == null) {
      call.reject("No context");
      return;
    }
    stopLanSignalingServerInternal();
    try {
      acquireWifiLocks(ctx);
      lanSignalingServer = new LanSignalingWebSocketServer(roomId, 0);
      lanSignalingServer.start();
      int port = lanSignalingServer.getPort();
      JSObject ret = new JSObject();
      ret.put("port", port);
      call.resolve(ret);
    } catch (Exception e) {
      Log.e(TAG, "startLanSignalingServer", e);
      stopLanSignalingServerInternal();
      call.reject("startLanSignalingServer failed", e);
    }
  }

  @PluginMethod
  public void stopLanSignalingServer(PluginCall call) {
    stopLanSignalingServerInternal();
    call.resolve();
  }

  private void acquireWifiLocks(Context ctx) {
    releaseWifiLocks();
    WifiManager wm = (WifiManager) ctx.getApplicationContext().getSystemService(Context.WIFI_SERVICE);
    if (wm == null) {
      return;
    }
    int mode =
        Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q
            ? WifiManager.WIFI_MODE_FULL_HIGH_PERF
            : WifiManager.WIFI_MODE_FULL;
    wifiLock = wm.createWifiLock(mode, "GuardedSleepLanSignaling");
    wifiLock.setReferenceCounted(false);
    wifiLock.acquire();
    multicastLock = wm.createMulticastLock("GuardedSleepLanNsd");
    multicastLock.setReferenceCounted(false);
    multicastLock.acquire();
  }

  private void releaseWifiLocks() {
    if (multicastLock != null) {
      try {
        if (multicastLock.isHeld()) {
          multicastLock.release();
        }
      } catch (RuntimeException ignored) {
      }
      multicastLock = null;
    }
    if (wifiLock != null) {
      try {
        if (wifiLock.isHeld()) {
          wifiLock.release();
        }
      } catch (RuntimeException ignored) {
      }
      wifiLock = null;
    }
  }

  private void stopLanSignalingServerInternal() {
    if (lanSignalingServer != null) {
      try {
        lanSignalingServer.stop(2000);
      } catch (Exception e) {
        Log.w(TAG, "lan signaling stop", e);
      }
      lanSignalingServer = null;
    }
    releaseWifiLocks();
  }

  @PluginMethod
  public void startNsdAdvertise(PluginCall call) {
    int port = call.getInt("port", 9);
    String discoveryToken = call.getString("discoveryToken", "");
    String legacyRoomId = call.getString("roomId", "");
    if (discoveryToken.isEmpty() && !legacyRoomId.isEmpty()) {
      discoveryToken = UUID.randomUUID().toString().replace("-", "").substring(0, 16);
    }
    if (discoveryToken.isEmpty()) {
      call.reject("discoveryToken or roomId required");
      return;
    }
    Context ctx = getContext();
    if (ctx == null) {
      call.reject("No context");
      return;
    }
    stopNsdAdvertiseInternal();
    nsdManager = (NsdManager) ctx.getSystemService(Context.NSD_SERVICE);
    if (nsdManager == null) {
      call.reject("NSD not available");
      return;
    }

    NsdServiceInfo info = new NsdServiceInfo();
    String name = sanitizeServiceName("gst-" + discoveryToken);
    info.setServiceName(name);
    info.setServiceType(SERVICE_TYPE);
    info.setPort(port);

    registrationListener =
        new NsdManager.RegistrationListener() {
          @Override
          public void onRegistrationFailed(NsdServiceInfo serviceInfo, int errorCode) {
            Log.w(TAG, "NSD register failed: " + errorCode);
          }

          @Override
          public void onUnregistrationFailed(NsdServiceInfo serviceInfo, int errorCode) {
            Log.w(TAG, "NSD unregister failed: " + errorCode);
          }

          @Override
          public void onServiceRegistered(NsdServiceInfo serviceInfo) {
            Log.i(TAG, "NSD registered " + serviceInfo.getServiceName());
          }

          @Override
          public void onServiceUnregistered(NsdServiceInfo serviceInfo) {
            Log.i(TAG, "NSD unregistered");
          }
        };

    try {
      nsdManager.registerService(info, NsdManager.PROTOCOL_DNS_SD, registrationListener);
      advertising = true;
      call.resolve();
    } catch (Exception e) {
      Log.e(TAG, "registerService", e);
      call.reject("registerService failed", e);
    }
  }

  @PluginMethod
  public void stopNsdAdvertise(PluginCall call) {
    stopNsdAdvertiseInternal();
    call.resolve();
  }

  private void stopNsdAdvertiseInternal() {
    if (nsdManager != null && registrationListener != null && advertising) {
      try {
        nsdManager.unregisterService(registrationListener);
      } catch (Exception e) {
        Log.w(TAG, "unregisterService", e);
      }
    }
    advertising = false;
    registrationListener = null;
  }

  @PluginMethod
  public void startNsdDiscovery(PluginCall call) {
    Context ctx = getContext();
    if (ctx == null) {
      call.reject("No context");
      return;
    }
    stopNsdDiscoveryInternal();
    nsdManager = (NsdManager) ctx.getSystemService(Context.NSD_SERVICE);
    if (nsdManager == null) {
      call.reject("NSD not available");
      return;
    }

    discoveryListener =
        new NsdManager.DiscoveryListener() {
          @Override
          public void onStartDiscoveryFailed(String serviceType, int errorCode) {
            Log.w(TAG, "discovery start failed " + errorCode);
          }

          @Override
          public void onStopDiscoveryFailed(String serviceType, int errorCode) {
            Log.w(TAG, "discovery stop failed " + errorCode);
          }

          @Override
          public void onDiscoveryStarted(String serviceType) {
            Log.i(TAG, "NSD discovery started");
          }

          @Override
          public void onDiscoveryStopped(String serviceType) {
            Log.i(TAG, "NSD discovery stopped");
          }

          @Override
          public void onServiceFound(NsdServiceInfo serviceInfo) {
            if (!SERVICE_TYPE.equals(serviceInfo.getServiceType())) return;
            nsdManager.resolveService(
                serviceInfo,
                new NsdManager.ResolveListener() {
                  @Override
                  public void onResolveFailed(NsdServiceInfo serviceInfo, int errorCode) {
                    Log.w(TAG, "resolve failed " + errorCode);
                  }

                  @Override
                  public void onServiceResolved(NsdServiceInfo info) {
                    try {
                      if (info.getHost() == null) return;
                      String host = info.getHost().getHostAddress();
                      int port = info.getPort();
                      String room = "";
                      String token = "";
                      if (info.getServiceName() != null) {
                        String n = info.getServiceName();
                        if (n.startsWith("gst-")) {
                          token = n.substring("gst-".length());
                        } else if (n.startsWith("gssleep-")) {
                          room = n.substring("gssleep-".length());
                        }
                      }
                      JSObject ev = new JSObject();
                      ev.put("room", room);
                      ev.put("token", token);
                      ev.put("host", host);
                      ev.put("port", port);
                      notifyListeners("lanPeerResolved", ev);
                    } catch (Exception e) {
                      Log.w(TAG, "onServiceResolved", e);
                    }
                  }
                });
          }

          @Override
          public void onServiceLost(NsdServiceInfo serviceInfo) {
            Log.i(TAG, "service lost");
          }
        };

    try {
      nsdManager.discoverServices(SERVICE_TYPE, NsdManager.PROTOCOL_DNS_SD, discoveryListener);
      discovering = true;
      call.resolve();
    } catch (Exception e) {
      Log.e(TAG, "discoverServices", e);
      call.reject("discoverServices failed", e);
    }
  }

  @PluginMethod
  public void stopNsdDiscovery(PluginCall call) {
    stopNsdDiscoveryInternal();
    call.resolve();
  }

  private void stopNsdDiscoveryInternal() {
    if (nsdManager != null && discoveryListener != null && discovering) {
      try {
        nsdManager.stopServiceDiscovery(discoveryListener);
      } catch (Exception e) {
        Log.w(TAG, "stopServiceDiscovery", e);
      }
    }
    discovering = false;
    discoveryListener = null;
  }

  private static String sanitizeServiceName(String name) {
    String s = name.replaceAll("[^A-Za-z0-9\\-_]", "-");
    if (s.length() > 63) s = s.substring(0, 63);
    return s.isEmpty() ? "gssleep" : s;
  }

  @Override
  protected void handleOnDestroy() {
    immersiveModeWanted = false;
    stopLanSignalingServerInternal();
    stopNsdAdvertiseInternal();
    stopNsdDiscoveryInternal();
    super.handleOnDestroy();
  }
}
