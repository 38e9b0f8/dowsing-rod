package io.github.dowsingrod;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.util.Arrays;
import java.util.Collections;
import org.junit.jupiter.api.Test;

class ScanOptionsTest {
    @Test
    void unsetOptionsPassNoFlagsSoTheConfigFileApplies() {
        assertTrue(new ScanOptions().toArguments().isEmpty());
    }

    @Test
    void everyOptionMapsToItsCliFlag() {
        ScanOptions options = new ScanOptions()
                .minSimilarity(0.75)
                .normalization("Aggressive")
                .format("jsonl")
                .maxClusters(5)
                .maxTokens(4000)
                .jobs(2)
                .excludes(Arrays.asList("generated", "**/*.gen.java"))
                .includes(Collections.singletonList("src/**"))
                .useCache(false)
                .failOnError(true);
        assertEquals(Arrays.asList(
                "--format", "jsonl",
                "--min-similarity", "0.75",
                "--normalization", "aggressive",
                "--max-clusters", "5",
                "--max-tokens", "4000",
                "--jobs", "2",
                "--exclude", "generated", "--exclude", "**/*.gen.java",
                "--include", "src/**",
                "--no-cache",
                "--fail-on-error"), options.toArguments());
    }

    @Test
    void aiWinsOverFormatAndAiFormatBecomesTheFlag() {
        assertEquals(Collections.singletonList("--ai"), new ScanOptions().format("json").ai(true).toArguments());
        assertEquals(Collections.singletonList("--ai"), new ScanOptions().format("ai").toArguments());
        assertEquals(Arrays.asList("--format", "json"),
                new ScanOptions().ai(true).withFormat("json").toArguments());
    }

    @Test
    void explicitFalseValuesAreStillPassed() {
        assertEquals(Collections.singletonList("--no-fail-on-error"),
                new ScanOptions().failOnError(false).toArguments());
        assertTrue(new ScanOptions().useCache(true).toArguments().isEmpty());
    }

    @Test
    void invalidValuesFailEarly() {
        assertThrows(IllegalArgumentException.class, () -> new ScanOptions().minSimilarity(1.5));
        assertThrows(IllegalArgumentException.class, () -> new ScanOptions().minSimilarity(Double.NaN));
        assertThrows(IllegalArgumentException.class, () -> new ScanOptions().normalization("loose"));
        assertThrows(IllegalArgumentException.class, () -> new ScanOptions().format("xml"));
        assertThrows(IllegalArgumentException.class, () -> new ScanOptions().jobs(0));
        assertThrows(IllegalArgumentException.class, () -> new ScanOptions().maxClusters(-1));
    }
}
