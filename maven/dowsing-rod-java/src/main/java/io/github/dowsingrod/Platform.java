package io.github.dowsingrod;

import java.io.File;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Locale;

/**
 * A platform with a prebuilt {@code dowsing-rod} executable. Classifiers follow the
 * os-maven-plugin convention ({@code linux-x86_64}, {@code osx-aarch_64}, ...), with a
 * {@code -musl} suffix for static Linux builds used on Alpine and other musl systems.
 */
public final class Platform {
    /** Every classifier that a release publishes. */
    public static final List<String> CLASSIFIERS = Collections.unmodifiableList(Arrays.asList(
            "linux-x86_64",
            "linux-aarch_64",
            "linux-x86_64-musl",
            "linux-aarch_64-musl",
            "osx-x86_64",
            "osx-aarch_64",
            "windows-x86_64"));

    private final String classifier;
    private final boolean windows;

    private Platform(String classifier, boolean windows) {
        this.classifier = classifier;
        this.windows = windows;
    }

    /** Detects the platform of the running JVM. */
    public static Platform current() {
        return detect(System.getProperty("os.name", ""), System.getProperty("os.arch", ""), isMusl());
    }

    /** Detects a platform from {@code os.name} and {@code os.arch} values. */
    public static Platform detect(String osName, String osArch, boolean musl) {
        String os = osName.toLowerCase(Locale.ROOT);
        String arch = normalizeArch(osArch.toLowerCase(Locale.ROOT));
        String family;
        if (os.startsWith("windows")) {
            family = "windows";
        } else if (os.startsWith("mac") || os.startsWith("darwin")) {
            family = "osx";
        } else if (os.startsWith("linux")) {
            family = "linux";
        } else {
            family = null;
        }
        String classifier = family == null || arch == null
                ? null
                : family + "-" + arch + ("linux".equals(family) && musl ? "-musl" : "");
        if (classifier == null || !CLASSIFIERS.contains(classifier)) {
            throw new DowsingRodException("dowsing-rod has no prebuilt binary for " + osName + "/" + osArch
                    + (musl ? " (musl)" : "") + ". Build it from source (cargo install --path crates/dowsing-cli)"
                    + " and point to it with binaryPath / -Ddowsing-rod.binaryPath.");
        }
        return new Platform(classifier, "windows".equals(family));
    }

    private static String normalizeArch(String arch) {
        switch (arch) {
            case "amd64":
            case "x86_64":
            case "x64":
                return "x86_64";
            case "aarch64":
            case "arm64":
                return "aarch_64";
            default:
                return null;
        }
    }

    /** musl systems ship the dynamic loader as /lib/ld-musl-*.so.1 instead of glibc's. */
    static boolean isMusl() {
        if (!System.getProperty("os.name", "").toLowerCase(Locale.ROOT).startsWith("linux")) {
            return false;
        }
        File[] loaders = new File("/lib").listFiles((dir, name) -> name.startsWith("ld-musl-"));
        return loaders != null && loaders.length > 0;
    }

    /** The Maven classifier of the native artifact for this platform. */
    public String classifier() {
        return classifier;
    }

    /** {@code dowsing-rod.exe} on Windows, {@code dowsing-rod} elsewhere. */
    public String executableName() {
        return windows ? "dowsing-rod.exe" : "dowsing-rod";
    }

    /** Path of the executable inside the classifier jar (and on the classpath). */
    public String resourcePath() {
        return "native/" + classifier + "/" + executableName();
    }

    public boolean isWindows() {
        return windows;
    }

    @Override
    public String toString() {
        return classifier;
    }
}
