# AndroidX Core
-keep class androidx.core.app.CoreComponentFactory { *; }
-keep class androidx.core.** { *; }
-keep public class * extends androidx.core.app.CoreComponentFactory
-keep public class * extends androidx.app.AppComponentFactory

# Flutter Wrapper
-keep class io.flutter.app.** { *; }
-keep class io.flutter.plugin.** { *; }
-keep class io.flutter.util.** { *; }
-keep class io.flutter.view.** { *; }
-keep class io.flutter.** { *; }
-keep class io.flutter.plugins.** { *; }

# Audio Service
-keep class com.ryanheise.audioservice.** { *; }

# Just Audio
-keep class com.ryanheise.just_audio.** { *; }
-keep class com.google.android.exoplayer2.** { *; }

# Google Fonts
-keep class com.google.fonts.** { *; }

# Maintain Metadata
-keepattributes Exceptions,InnerClasses,Signature,Deprecated,SourceFile,LineNumberTable,*Annotation*,EnclosingMethod

# Fix for Play Core missing classes
-dontwarn com.google.android.play.core.**
-keep class com.google.android.play.core.** { *; }