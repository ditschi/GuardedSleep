# R8 keep rules for the Rust core bindings (applied to every app that links this module).
#
# UniFFI: the generated classes are reached from native code and through JNA reflection (callback
# interfaces, Structure fields, enum and record constructors), so R8 must neither rename nor strip them.
-keep class uniffi.** { *; }

# JNA Structure.FieldOrder and generic callback signatures are read through reflection: do not depend on the
# consumer's default ProGuard file to keep these attributes.
-keepattributes *Annotation*,Signature,InnerClasses,EnclosingMethod

# JNA looks up its own classes, Structure fields and the Native/Callback methods by name from
# libjnidispatch.so. The AAR ships no rules. Doc: https://github.com/java-native-access/jna/blob/master/www/FrequentlyAskedQuestions.md
-keep class com.sun.jna.** { *; }
-keepclassmembers class * extends com.sun.jna.** { public *; }

# JNA's desktop-only helpers reference java.awt, which Android does not have.
-dontwarn java.awt.**
