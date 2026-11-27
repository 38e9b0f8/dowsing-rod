package io.github.dowsingrod.maven;

import io.github.dowsingrod.ScanOptions;
import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;
import org.apache.maven.plugin.MojoExecutionException;
import org.apache.maven.plugins.annotations.Parameter;

/**
 * Scan options. None has a default value: an option you leave unset is not passed to the scanner,
 * so {@code dowsing-rod.toml} decides it. An option you set always overrides the file, even when
 * it equals the built-in default, exactly like the CLI flags.
 */
abstract class AbstractScanMojo extends AbstractDowsingRodMojo {
    /** Minimum similarity (0.0–1.0) for two functions to count as related. Built-in default 0.75. */
    @Parameter(property = "dowsing-rod.minSimilarity")
    protected Double minSimilarity;

    /** {@code strict}, {@code balanced} (built-in default), or {@code aggressive}. */
    @Parameter(property = "dowsing-rod.normalization")
    protected String normalization;

    /** {@code human} (built-in default), {@code ai}, {@code json}, or {@code jsonl}. */
    @Parameter(property = "dowsing-rod.format")
    protected String format;

    /** Compact output for coding agents; same as {@code format=ai}. */
    @Parameter(property = "dowsing-rod.ai")
    protected Boolean ai;

    /** Report at most this many clusters, most valuable first. */
    @Parameter(property = "dowsing-rod.maxClusters")
    protected Integer maxClusters;

    /** Token budget for AI output. */
    @Parameter(property = "dowsing-rod.maxTokens")
    protected Integer maxTokens;

    /** Scanner worker threads. */
    @Parameter(property = "dowsing-rod.jobs")
    protected Integer jobs;

    /** Extra paths or globs to skip. Build output, {@code .gitignore}d files and VCS folders are skipped already. */
    @Parameter(property = "dowsing-rod.excludes")
    protected List<String> excludes;

    /** Globs that override excludes. */
    @Parameter(property = "dowsing-rod.includes")
    protected List<String> includes;

    /** Set to {@code false} to disable the {@code .dowsing-rod-cache/} file analysis cache. */
    @Parameter(property = "dowsing-rod.useCache")
    protected Boolean useCache;

    /** Fail the build when any file cannot be parsed ({@code false} overrides {@code fail_on_error = true}). */
    @Parameter(property = "dowsing-rod.failOnError")
    protected Boolean failOnError;

    /** Also write the report to this file, for CI to archive. */
    @Parameter(property = "dowsing-rod.outputFile")
    protected File outputFile;

    protected ScanOptions options() throws MojoExecutionException {
        try {
            return new ScanOptions()
                    .minSimilarity(minSimilarity)
                    .normalization(normalization)
                    .format(format)
                    .ai(ai)
                    .maxClusters(maxClusters)
                    .maxTokens(maxTokens)
                    .jobs(jobs)
                    .excludes(excludes)
                    .includes(includes)
                    .useCache(useCache)
                    .failOnError(failOnError);
        } catch (IllegalArgumentException e) {
            throw new MojoExecutionException("Invalid dowsing-rod configuration: " + e.getMessage(), e);
        }
    }

    protected void write(Path file, String content) throws MojoExecutionException {
        try {
            Path absolute = file.toAbsolutePath();
            if (absolute.getParent() != null) {
                Files.createDirectories(absolute.getParent());
            }
            Files.write(absolute, content.getBytes(StandardCharsets.UTF_8));
            getLog().info("dowsing-rod report written to " + absolute);
        } catch (IOException e) {
            throw new MojoExecutionException("Cannot write " + file + ": " + e.getMessage(), e);
        }
    }

    /** Resolve {@link #outputFile} against the project folder, as Maven does for paths. */
    protected Path outputPath() {
        if (outputFile == null) {
            return null;
        }
        if (outputFile.isAbsolute() || project.getBasedir() == null) {
            return outputFile.toPath();
        }
        return project.getBasedir().toPath().resolve(outputFile.toPath());
    }
}
