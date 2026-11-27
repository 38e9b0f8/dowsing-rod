package io.github.dowsingrod;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Locale;

/**
 * Options for {@code dowsing-rod scan}. Every option is unset by default, and an unset option is
 * never passed to the CLI, so the project's {@code dowsing-rod.toml} applies to it. A set option
 * always overrides the file, even when it equals the built-in default.
 */
public final class ScanOptions {
    private Double minSimilarity;
    private String normalization;
    private String format;
    private Boolean ai;
    private Integer maxClusters;
    private Integer maxTokens;
    private Integer jobs;
    private List<String> excludes = Collections.emptyList();
    private List<String> includes = Collections.emptyList();
    private Boolean useCache;
    private Boolean failOnError;

    public ScanOptions minSimilarity(Double value) {
        if (value != null && (value.isNaN() || value < 0.0 || value > 1.0)) {
            throw new IllegalArgumentException("minSimilarity must be between 0 and 1, got " + value);
        }
        this.minSimilarity = value;
        return this;
    }

    /** {@code strict}, {@code balanced}, or {@code aggressive}. */
    public ScanOptions normalization(String value) {
        this.normalization = choice("normalization", value, "strict", "balanced", "aggressive");
        return this;
    }

    /** {@code human}, {@code ai}, {@code json}, or {@code jsonl}. */
    public ScanOptions format(String value) {
        this.format = choice("format", value, "human", "ai", "json", "jsonl");
        return this;
    }

    /** {@code --ai}; overrides {@link #format(String)}. */
    public ScanOptions ai(Boolean value) {
        this.ai = value;
        return this;
    }

    public ScanOptions maxClusters(Integer value) {
        this.maxClusters = positive("maxClusters", value, 0);
        return this;
    }

    public ScanOptions maxTokens(Integer value) {
        this.maxTokens = positive("maxTokens", value, 0);
        return this;
    }

    public ScanOptions jobs(Integer value) {
        this.jobs = positive("jobs", value, 1);
        return this;
    }

    public ScanOptions excludes(List<String> value) {
        this.excludes = value == null ? Collections.<String>emptyList() : new ArrayList<>(value);
        return this;
    }

    public ScanOptions includes(List<String> value) {
        this.includes = value == null ? Collections.<String>emptyList() : new ArrayList<>(value);
        return this;
    }

    /** {@code false} passes {@code --no-cache}. */
    public ScanOptions useCache(Boolean value) {
        this.useCache = value;
        return this;
    }

    /** {@code true} passes {@code --fail-on-error}, {@code false} {@code --no-fail-on-error}. */
    public ScanOptions failOnError(Boolean value) {
        this.failOnError = value;
        return this;
    }

    /** A copy of these options with the output format forced, for callers that parse it. */
    ScanOptions withFormat(String value) {
        ScanOptions copy = copy();
        copy.ai = null;
        copy.format = value;
        return copy;
    }

    private ScanOptions copy() {
        ScanOptions copy = new ScanOptions();
        copy.minSimilarity = minSimilarity;
        copy.normalization = normalization;
        copy.format = format;
        copy.ai = ai;
        copy.maxClusters = maxClusters;
        copy.maxTokens = maxTokens;
        copy.jobs = jobs;
        copy.excludes = excludes;
        copy.includes = includes;
        copy.useCache = useCache;
        copy.failOnError = failOnError;
        return copy;
    }

    /** The CLI flags for these options, without the {@code scan} command or path. */
    public List<String> toArguments() {
        List<String> args = new ArrayList<>();
        if (Boolean.TRUE.equals(ai) || "ai".equals(format)) {
            // `ai` is a config-file format; on the command line it is the --ai flag.
            args.add("--ai");
        } else if (format != null) {
            args.add("--format");
            args.add(format);
        }
        if (minSimilarity != null) {
            args.add("--min-similarity");
            args.add(String.valueOf(minSimilarity));
        }
        if (normalization != null) {
            args.add("--normalization");
            args.add(normalization);
        }
        if (maxClusters != null) {
            args.add("--max-clusters");
            args.add(String.valueOf(maxClusters));
        }
        if (maxTokens != null) {
            args.add("--max-tokens");
            args.add(String.valueOf(maxTokens));
        }
        if (jobs != null) {
            args.add("--jobs");
            args.add(String.valueOf(jobs));
        }
        for (String exclude : excludes) {
            args.add("--exclude");
            args.add(exclude);
        }
        for (String include : includes) {
            args.add("--include");
            args.add(include);
        }
        if (Boolean.FALSE.equals(useCache)) {
            args.add("--no-cache");
        }
        if (failOnError != null) {
            args.add(failOnError ? "--fail-on-error" : "--no-fail-on-error");
        }
        return args;
    }

    private static String choice(String name, String value, String... allowed) {
        if (value == null) {
            return null;
        }
        String normalized = value.trim().toLowerCase(Locale.ROOT);
        for (String option : allowed) {
            if (option.equals(normalized)) {
                return normalized;
            }
        }
        throw new IllegalArgumentException(name + " must be one of " + String.join(", ", allowed) + ", got " + value);
    }

    private static Integer positive(String name, Integer value, int minimum) {
        if (value != null && value < minimum) {
            throw new IllegalArgumentException(name + " must be at least " + minimum + ", got " + value);
        }
        return value;
    }
}
