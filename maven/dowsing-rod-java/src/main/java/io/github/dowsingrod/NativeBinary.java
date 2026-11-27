package io.github.dowsingrod;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.AtomicMoveNotSupportedException;
import java.nio.file.FileSystemException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardCopyOption;
import java.nio.file.attribute.PosixFilePermission;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.EnumSet;
import java.util.Locale;
import java.util.Set;
import java.util.zip.ZipEntry;
import java.util.zip.ZipFile;

/**
 * Extracts the native executable from a {@code dowsing-rod-native} classifier jar (or the
 * classpath) into a cache directory, verifying its SHA-256 before it is ever run.
 *
 * <p>The expected checksum comes from this library's own jar, which is built together with the
 * native jars of the same version. A dev build that lacks a platform's checksum falls back to the
 * checksum stored next to the binary inside the native jar.
 */
public final class NativeBinary {
    private static final Set<PosixFilePermission> EXECUTABLE = EnumSet.of(
            PosixFilePermission.OWNER_READ, PosixFilePermission.OWNER_WRITE, PosixFilePermission.OWNER_EXECUTE,
            PosixFilePermission.GROUP_READ, PosixFilePermission.GROUP_EXECUTE,
            PosixFilePermission.OTHERS_READ, PosixFilePermission.OTHERS_EXECUTE);

    private NativeBinary() {}

    /**
     * The default extraction directory: {@code -Ddowsing-rod.cacheDir}, else
     * {@code ~/.dowsing-rod/native}. It is never the system temp directory, which hardened CI
     * agents often mount {@code noexec}.
     */
    public static Path defaultCacheDirectory() {
        String configured = System.getProperty("dowsing-rod.cacheDir");
        if (configured != null && !configured.trim().isEmpty()) {
            return Paths.get(configured);
        }
        return Paths.get(System.getProperty("user.home"), ".dowsing-rod", "native");
    }

    /** Extract the executable for {@code platform} from a classifier jar. */
    public static Path extractFromJar(Path jar, Platform platform, Path cacheDirectory) {
        try (ZipFile zip = new ZipFile(jar.toFile())) {
            ZipEntry entry = zip.getEntry(platform.resourcePath());
            if (entry == null) {
                throw new DowsingRodException(jar + " does not contain " + platform.resourcePath()
                        + "; it is not the " + platform.classifier() + " dowsing-rod-native jar");
            }
            String expected = expectedChecksum(platform);
            if (expected == null) {
                ZipEntry sidecar = zip.getEntry(platform.resourcePath() + ".sha256");
                if (sidecar != null) {
                    try (InputStream in = zip.getInputStream(sidecar)) {
                        expected = readChecksum(in);
                    }
                }
            }
            if (expected == null) {
                throw new DowsingRodException("no SHA-256 is known for the " + platform.classifier()
                        + " dowsing-rod binary; refusing to run an unverified executable");
            }
            try (InputStream in = zip.getInputStream(entry)) {
                return install(in, expected, platform, cacheDirectory);
            }
        } catch (IOException e) {
            throw new DowsingRodException("cannot read " + jar + ": " + e.getMessage(), e);
        }
    }

    /**
     * Extract the executable from the classpath, where a {@code dowsing-rod-native} classifier jar
     * was added as a dependency. Returns {@code null} when it is not on the classpath.
     */
    public static Path extractFromClasspath(ClassLoader loader, Platform platform, Path cacheDirectory) {
        String expected = expectedChecksum(platform);
        try {
            if (expected == null) {
                try (InputStream in = loader.getResourceAsStream(platform.resourcePath() + ".sha256")) {
                    expected = in == null ? null : readChecksum(in);
                }
            }
            try (InputStream in = loader.getResourceAsStream(platform.resourcePath())) {
                if (in == null) {
                    return null;
                }
                if (expected == null) {
                    throw new DowsingRodException("no SHA-256 is known for the " + platform.classifier()
                            + " dowsing-rod binary; refusing to run an unverified executable");
                }
                return install(in, expected, platform, cacheDirectory);
            }
        } catch (IOException e) {
            throw new DowsingRodException("cannot read " + platform.resourcePath() + " from the classpath", e);
        }
    }

    /** SHA-256 recorded in this library at build time for {@code platform}, or {@code null}. */
    static String expectedChecksum(Platform platform) {
        String resource = "/META-INF/dowsing-rod/checksums/" + platform.classifier() + ".sha256";
        try (InputStream in = NativeBinary.class.getResourceAsStream(resource)) {
            return in == null ? null : readChecksum(in);
        } catch (IOException e) {
            return null;
        }
    }

    private static Path install(InputStream content, String expected, Platform platform, Path cacheDirectory)
            throws IOException {
        Path directory = cacheDirectory.resolve(Version.get()).resolve(platform.classifier());
        Path target = directory.resolve(platform.executableName());
        // Reuse an earlier extraction, but only if it is still byte-for-byte what we expect.
        if (Files.isRegularFile(target) && expected.equals(sha256(target))) {
            makeExecutable(target);
            return target;
        }
        Files.createDirectories(directory);
        Path temporary = Files.createTempFile(directory, platform.executableName(), ".tmp");
        try {
            MessageDigest digest = sha256();
            try (OutputStream out = Files.newOutputStream(temporary)) {
                byte[] buffer = new byte[64 * 1024];
                int read;
                while ((read = content.read(buffer)) != -1) {
                    digest.update(buffer, 0, read);
                    out.write(buffer, 0, read);
                }
            }
            String actual = hex(digest.digest());
            if (!expected.equals(actual)) {
                throw new DowsingRodException("SHA-256 mismatch for the " + platform.classifier()
                        + " dowsing-rod binary: expected " + expected + ", found " + actual);
            }
            makeExecutable(temporary);
            try {
                Files.move(temporary, target, StandardCopyOption.ATOMIC_MOVE, StandardCopyOption.REPLACE_EXISTING);
            } catch (AtomicMoveNotSupportedException e) {
                Files.move(temporary, target, StandardCopyOption.REPLACE_EXISTING);
            } catch (FileSystemException e) {
                // Windows: a concurrent build may already run (and lock) an identical copy.
                if (!(Files.isRegularFile(target) && expected.equals(sha256(target)))) {
                    throw e;
                }
            }
            return target;
        } finally {
            Files.deleteIfExists(temporary);
        }
    }

    private static void makeExecutable(Path file) throws IOException {
        if (System.getProperty("os.name", "").toLowerCase(Locale.ROOT).startsWith("windows")) {
            return;
        }
        try {
            if (!Files.getPosixFilePermissions(file).containsAll(EXECUTABLE)) {
                Files.setPosixFilePermissions(file, EXECUTABLE);
            }
        } catch (UnsupportedOperationException e) {
            if (!file.toFile().setExecutable(true, false)) {
                throw new IOException("cannot mark " + file + " executable", e);
            }
        }
    }

    static String sha256(Path file) throws IOException {
        MessageDigest digest = sha256();
        try (InputStream in = Files.newInputStream(file)) {
            byte[] buffer = new byte[64 * 1024];
            int read;
            while ((read = in.read(buffer)) != -1) {
                digest.update(buffer, 0, read);
            }
        }
        return hex(digest.digest());
    }

    private static MessageDigest sha256() {
        try {
            return MessageDigest.getInstance("SHA-256");
        } catch (NoSuchAlgorithmException e) {
            throw new IllegalStateException("SHA-256 is required by every Java platform", e);
        }
    }

    private static String readChecksum(InputStream in) throws IOException {
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        byte[] buffer = new byte[256];
        int read;
        while ((read = in.read(buffer)) != -1) {
            out.write(buffer, 0, read);
        }
        String text = new String(out.toByteArray(), StandardCharsets.US_ASCII).trim();
        int space = text.indexOf(' ');
        String digest = (space < 0 ? text : text.substring(0, space)).toLowerCase(Locale.ROOT);
        return digest.matches("[0-9a-f]{64}") ? digest : null;
    }

    private static String hex(byte[] bytes) {
        StringBuilder out = new StringBuilder(bytes.length * 2);
        for (byte b : bytes) {
            out.append(Character.forDigit((b >> 4) & 0xf, 16)).append(Character.forDigit(b & 0xf, 16));
        }
        return out.toString();
    }
}
