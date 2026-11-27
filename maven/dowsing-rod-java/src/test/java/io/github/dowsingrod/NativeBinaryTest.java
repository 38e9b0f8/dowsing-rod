package io.github.dowsingrod;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;
import org.junit.jupiter.api.Assumptions;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/**
 * Builds fake classifier jars whose sidecar checksum the test controls. That path is only taken
 * when dowsing-rod-java has no embedded checksum for the platform, so the test uses a platform
 * other than the host's (a dev or CI build embeds only the staged host checksum). In a release
 * build, every checksum is embedded and these tests are skipped; DowsingRodTest covers that path.
 */
class NativeBinaryTest {
    private static final Platform PLATFORM = platformWithoutEmbeddedChecksum();
    private static final String DIRECTORY = PLATFORM == null ? "" : PLATFORM.classifier();

    private static Platform platformWithoutEmbeddedChecksum() {
        String[][] candidates = {{"Windows 11", "amd64"}, {"Linux", "amd64"}, {"Mac OS X", "aarch64"}};
        for (String[] candidate : candidates) {
            Platform platform = Platform.detect(candidate[0], candidate[1], false);
            if (NativeBinary.expectedChecksum(platform) == null) {
                return platform;
            }
        }
        return null;
    }
    private static final byte[] CONTENT = "#!/bin/sh\necho fake\n".getBytes(StandardCharsets.US_ASCII);

    @TempDir
    Path temp;

    private Path jar(byte[] content, String checksum) throws IOException {
        Path jar = temp.resolve("native-" + System.nanoTime() + ".jar");
        try (OutputStream file = Files.newOutputStream(jar); ZipOutputStream zip = new ZipOutputStream(file)) {
            zip.putNextEntry(new ZipEntry(PLATFORM.resourcePath()));
            zip.write(content);
            zip.closeEntry();
            if (checksum != null) {
                zip.putNextEntry(new ZipEntry(PLATFORM.resourcePath() + ".sha256"));
                zip.write((checksum + "\n").getBytes(StandardCharsets.US_ASCII));
                zip.closeEntry();
            }
        }
        return jar;
    }

    private static String sha256(byte[] content, Path temp) throws IOException {
        Path file = Files.createTempFile(temp, "content", ".bin");
        Files.write(file, content);
        return NativeBinary.sha256(file);
    }

    private static void assumeSidecarPath() {
        Assumptions.assumeTrue(PLATFORM != null, "every checksum is embedded (release build)");
    }

    @Test
    void extractsVerifiesAndReusesTheBinary() throws IOException {
        assumeSidecarPath();
        Path cache = temp.resolve("cache");
        Path first = NativeBinary.extractFromJar(jar(CONTENT, sha256(CONTENT, temp)), PLATFORM, cache);
        assertArrayEquals(CONTENT, Files.readAllBytes(first));
        assertEquals(cache.resolve(Version.get()).resolve(DIRECTORY).resolve(PLATFORM.executableName()), first);
        long modified = Files.getLastModifiedTime(first).toMillis();
        Path second = NativeBinary.extractFromJar(jar(CONTENT, sha256(CONTENT, temp)), PLATFORM, cache);
        assertEquals(first, second);
        assertEquals(modified, Files.getLastModifiedTime(second).toMillis());
    }

    @Test
    void replacesATamperedCachedCopy() throws IOException {
        assumeSidecarPath();
        Path cache = temp.resolve("cache");
        Path extracted = NativeBinary.extractFromJar(jar(CONTENT, sha256(CONTENT, temp)), PLATFORM, cache);
        Files.write(extracted, "tampered".getBytes(StandardCharsets.US_ASCII));
        Path again = NativeBinary.extractFromJar(jar(CONTENT, sha256(CONTENT, temp)), PLATFORM, cache);
        assertArrayEquals(CONTENT, Files.readAllBytes(again));
    }

    @Test
    void refusesMismatchedOrMissingChecksums() throws IOException {
        assumeSidecarPath();
        Path cache = temp.resolve("cache");
        String wrong = sha256("other".getBytes(StandardCharsets.US_ASCII), temp);
        DowsingRodException mismatch = assertThrows(DowsingRodException.class,
                () -> NativeBinary.extractFromJar(jar(CONTENT, wrong), PLATFORM, cache));
        assertTrue(mismatch.getMessage().contains("SHA-256 mismatch"), mismatch.getMessage());
        assertTrue(!Files.exists(cache.resolve(Version.get()).resolve(DIRECTORY).resolve(PLATFORM.executableName())));
        DowsingRodException missing = assertThrows(DowsingRodException.class,
                () -> NativeBinary.extractFromJar(jar(CONTENT, null), PLATFORM, cache));
        assertTrue(missing.getMessage().contains("unverified"), missing.getMessage());
    }

    @Test
    void rejectsTheWrongClassifierJar() throws IOException {
        assumeSidecarPath();
        Path jar = jar(CONTENT, sha256(CONTENT, temp));
        Platform other = Platform.detect("Linux", "aarch64", true);
        DowsingRodException error = assertThrows(DowsingRodException.class,
                () -> NativeBinary.extractFromJar(jar, other, temp.resolve("cache")));
        assertTrue(error.getMessage().contains("linux-aarch_64-musl"), error.getMessage());
    }

    @Test
    void classpathWithoutNativeJarReturnsNull() {
        ClassLoader empty = new ClassLoader(null) {};
        assertEquals(null, NativeBinary.extractFromClasspath(empty, Platform.current(), temp.resolve("cache")));
    }
}
