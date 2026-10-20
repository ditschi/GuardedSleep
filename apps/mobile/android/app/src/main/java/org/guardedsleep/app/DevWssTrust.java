package org.guardedsleep.app;

import java.net.InetAddress;
import java.net.UnknownHostException;
import java.security.SecureRandom;
import java.security.cert.X509Certificate;
import javax.net.ssl.HostnameVerifier;
import javax.net.ssl.HttpsURLConnection;
import javax.net.ssl.SSLContext;
import javax.net.ssl.SSLSession;
import javax.net.ssl.TrustManager;
import javax.net.ssl.X509TrustManager;
import okhttp3.OkHttpClient;

/**
 * Debug-only trust for self-signed WSS on private LAN hosts (Docker stack local dev).
 * Release builds never enable permissive trust ({@link BuildConfig#ALLOW_DEV_WSS_TRUST}).
 */
final class DevWssTrust {

  private DevWssTrust() {}

  static boolean isDevTrustEnabled() {
    return BuildConfig.DEBUG && BuildConfig.ALLOW_DEV_WSS_TRUST;
  }

  static boolean isPrivateLanHost(String host) {
    if (host == null || host.isEmpty()) {
      return false;
    }
    if ("localhost".equalsIgnoreCase(host)) {
      return true;
    }
    if ("127.0.0.1".equals(host) || "::1".equals(host) || "[::1]".equals(host)) {
      return true;
    }
    try {
      InetAddress addr = InetAddress.getByName(host);
      return addr.isSiteLocalAddress() || addr.isLoopbackAddress();
    } catch (UnknownHostException e) {
      return false;
    }
  }

  static OkHttpClient.Builder configureClient(OkHttpClient.Builder builder, String host) {
    if (!isDevTrustEnabled() || !isPrivateLanHost(host)) {
      return builder;
    }
    try {
      TrustManager[] trustManagers =
          new TrustManager[] {
            new X509TrustManager() {
              @Override
              public void checkClientTrusted(X509Certificate[] chain, String authType) {}

              @Override
              public void checkServerTrusted(X509Certificate[] chain, String authType) {}

              @Override
              public X509Certificate[] getAcceptedIssuers() {
                return new X509Certificate[0];
              }
            }
          };
      SSLContext sslContext = SSLContext.getInstance("TLS");
      sslContext.init(null, trustManagers, new SecureRandom());
      HostnameVerifier verifier =
          new HostnameVerifier() {
            @Override
            public boolean verify(String hostname, SSLSession session) {
              if (isPrivateLanHost(hostname)) {
                return true;
              }
              return HttpsURLConnection.getDefaultHostnameVerifier().verify(hostname, session);
            }
          };
      return builder
          .sslSocketFactory(sslContext.getSocketFactory(), (X509TrustManager) trustManagers[0])
          .hostnameVerifier(verifier);
    } catch (Exception e) {
      return builder;
    }
  }
}
