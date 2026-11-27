package io.github.dowsingrod;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.io.OutputStream;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.zip.ZipEntry;
import java.util.zip.ZipOutputStream;
import org.junit.jupiter.api.Assumptions;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;
import org.junit.jupiter.api.io.TempDir;

/**
 * End-to-end tests against the real binary staged by {@code scripts/package-maven.mjs host ...}.
 * They are skipped when no binary is staged, unless {@code -Ddowsing.requireNative=true} (set in
 * CI) makes a missing binary a failure.
 */
class DowsingRodTest {
    private static final Path REPO = Paths.get(System.getProperty("dowsing.repo.root", "../.."));
    private static Path staged;

    @TempDir
    Path temp;

    @BeforeAll
    static void findStagedBinary() {
        Platform platform = Platform.current();
        Path nativeDir = Paths.get(System.getProperty("dowsing.native.dir", "../dowsing-rod-native/native"));
        Path candidate = nativeDir.resolve(platform.classifier()).resolve(platform.resourcePath());
        if (Files.isRegularFile(candidate)) {
            staged = candidate;
        } else if (Boolean.getBoolean("dowsing.requireNative")) {
            throw new AssertionError("no native binary staged at " + candidate
                    + "; run node scripts/package-maven.mjs host <binary>");
        }
    }

    private static DowsingRod rod() {
        Assumptions.assumeTrue(staged != null, "no native binary staged for this platform");
        return new DowsingRod(staged);
    }

    @Test
    void mavenVersionIsTheCargoVersion() throws IOException {
        String cargo = new String(Files.readAllBytes(REPO.resolve("Cargo.toml")), StandardCharsets.UTF_8);
        Matcher matcher = Pattern.compile("(?m)^version = \"([^\"]+)\"").matcher(cargo);
        assertTrue(matcher.find());
        assertEquals(matcher.group(1), Version.get());
    }

    @Test
    void stagedBinaryReportsTheSameVersion() {
        assertEquals(Version.get(), rod().version());
    }

    @Test
    void scansEveryLanguageIncludingJava() {
        ScanResult result = rod().scan(REPO.resolve("tests/languages"),
                new ScanOptions().useCache(false).failOnError(true));
        assertEquals("1.1", result.schemaVersion());
        assertEquals(Version.get(), result.toolVersion());
        assertEquals(11, result.statistic("files_scanned"));
        assertEquals(43, result.functions().size());
        assertTrue(result.parseErrors().isEmpty());
        Set<Object> languages = new HashSet<>();
        for (Map<String, Object> function : result.functions()) {
            languages.add(function.get("language"));
        }
        assertTrue(languages.contains("java"), languages.toString());
        boolean javaCluster = false;
        for (ScanResult.Cluster cluster : result.clusters()) {
            javaCluster |= "java".equals(cluster.language());
        }
        assertTrue(javaCluster, "the Java fixture's duplicate methods should cluster");
    }

    @Test
    void javaProvidersProduceAStrategyCluster() {
        ScanResult result = rod().scan(REPO.resolve("tests/fixtures/java_providers"),
                new ScanOptions().useCache(false));
        ScanResult.Cluster charge = null;
        for (ScanResult.Cluster cluster : result.clusters()) {
            if (cluster.members().size() == 3 && cluster.members().get(0).contains("::charge:")) {
                charge = cluster;
            }
        }
        assertTrue(charge != null, "charge methods of the three gateways should cluster");
        assertFalse(charge.classification().isEmpty());
        assertTrue(charge.averageSimilarity() > 0.75);
    }

    @Test
    void streamsHumanAndAiOutput() {
        List<String> lines = new ArrayList<>();
        ProcessResult result = rod().run(Arrays.asList("scan", REPO.resolve("tests/languages").toString(),
                "--ai", "--max-tokens", "800", "--no-cache"), null, lines::add, null);
        assertEquals(0, result.exitCode());
        assertTrue(lines.get(0).startsWith("DOWSING-ROD v"), lines.get(0));
        assertTrue(lines.contains("LANGUAGE java") || String.join("\n", lines).contains("LANGUAGE"));
    }

    @Test
    void parseErrorsFailOnlyWhenAsked() throws IOException {
        DowsingRod rod = rod();
        Path source = temp.resolve("src");
        Files.createDirectories(source);
        Files.write(source.resolve("Broken.java"),
                "class Broken { int ok(int x) { return x; } int bad( { }".getBytes(StandardCharsets.UTF_8));
        ScanResult lenient = rod.scan(source, new ScanOptions().useCache(false));
        assertFalse(lenient.parseErrors().isEmpty());
        DowsingRodException strict = assertThrows(DowsingRodException.class,
                () -> rod.scan(source, new ScanOptions().useCache(false).failOnError(true)));
        assertTrue(strict.getMessage().contains("parse"), strict.getMessage());
    }

    @Test
    void usageErrorsExitWithTwo() {
        assertEquals(2, rod().run(Arrays.asList("--no-such-flag"), null, null, null).exitCode());
    }

    @Test
    void extractsTheStagedBinaryFromAClassifierJar() throws IOException {
        DowsingRod staged = rod();
        Platform platform = Platform.current();
        Path jar = temp.resolve("dowsing-rod-native.jar");
        try (OutputStream file = Files.newOutputStream(jar); ZipOutputStream zip = new ZipOutputStream(file)) {
            zip.putNextEntry(new ZipEntry(platform.resourcePath()));
            zip.write(Files.readAllBytes(staged.executable()));
            zip.closeEntry();
        }
        // No sidecar checksum in this jar: the checksum embedded in dowsing-rod-java must verify it.
        Path extracted = NativeBinary.extractFromJar(jar, platform, temp.resolve("cache"));
        assertEquals(Version.get(), new DowsingRod(extracted).version());
    }

    @Test
    void missingExecutableIsAClearError() {
        DowsingRodException error = assertThrows(DowsingRodException.class,
                () -> new DowsingRod(temp.resolve("nope")));
        assertTrue(error.getMessage().contains("not found"));
    }
}
