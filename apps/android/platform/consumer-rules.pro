# R8 keep rules for libwebrtc (io.github.webrtc-sdk:android; the AAR ships none). The native library
# (libjingle_peerconnection_so) registers JNI methods and calls back into the org.webrtc classes by
# name, so the whole package and every native method must survive shrinking and renaming.
-keep class org.webrtc.** { *; }
-keepclasseswithmembernames class org.webrtc.** { native <methods>; }
-dontwarn org.webrtc.**
