package io.github.dowsingrod;

import java.io.IOException;
import java.io.InputStream;
import java.util.Properties;

/** The Dowsing Rod version this library was built for (the version in Cargo.toml). */
public final class Version {
    /** Maven groupId of every Dowsing Rod artifact. */
    public static final String GROUP_ID = "io.github.shkb42";

    /** artifactId of the per-platform classifier jars holding the native executable. */
    public static final String NATIVE_ARTIFACT_ID = "dowsing-rod-native";

    private static final String VERSION = load();

    private Version() {}

    /** The version of this library, which is also the version of the native binaries it expects. */
    public static String get() {
        return VERSION;
    }

    private static String load() {
        try (InputStream in = Version.class.getResourceAsStream("/META-INF/dowsing-rod/version.properties")) {
            if (in == null) {
                throw new DowsingRodException("dowsing-rod-java is missing META-INF/dowsing-rod/version.properties");
            }
            Properties properties = new Properties();
            properties.load(in);
            return properties.getProperty("version");
        } catch (IOException e) {
            throw new DowsingRodException("cannot read the dowsing-rod-java version", e);
        }
    }
}
