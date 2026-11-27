package io.github.dowsingrod;

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStream;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.function.Consumer;

/**
 * Runs the native {@code dowsing-rod} executable. Analysis happens in that process, fully offline;
 * this class only builds arguments, streams output, and parses JSON.
 *
 * <pre>{@code
 * DowsingRod rod = DowsingRod.locate();
 * ScanResult result = rod.scan(Paths.get("."), new ScanOptions().minSimilarity(0.85));
 * for (ScanResult.Cluster cluster : result.clusters()) {
 *     System.out.println(cluster.id() + " " + cluster.classification() + " " + cluster.members());
 * }
 * }</pre>
 */
public final class DowsingRod {
    private final Path executable;

    /** Use a specific executable, such as one built from source. */
    public DowsingRod(Path executable) {
        if (!Files.isRegularFile(executable)) {
            throw new DowsingRodException("dowsing-rod executable not found: " + executable);
        }
        this.executable = executable;
    }

    /**
     * Find the executable: {@code -Ddowsing-rod.binaryPath}, then the {@code DOWSING_ROD_BINARY}
     * environment variable, then a {@code dowsing-rod-native} classifier jar for this platform on
     * the classpath (extracted to {@link NativeBinary#defaultCacheDirectory()}).
     */
    public static DowsingRod locate() {
        String configured = System.getProperty("dowsing-rod.binaryPath");
        if (configured == null || configured.trim().isEmpty()) {
            configured = System.getenv("DOWSING_ROD_BINARY");
        }
        if (configured != null && !configured.trim().isEmpty()) {
            return new DowsingRod(Paths.get(configured));
        }
        Platform platform = Platform.current();
        ClassLoader loader = DowsingRod.class.getClassLoader();
        Path extracted = NativeBinary.extractFromClasspath(
                loader == null ? ClassLoader.getSystemClassLoader() : loader, platform,
                NativeBinary.defaultCacheDirectory());
        if (extracted == null) {
            throw new DowsingRodException("no dowsing-rod executable found. Add the dependency "
                    + Version.GROUP_ID + ":" + Version.NATIVE_ARTIFACT_ID + ":" + Version.get() + ":"
                    + platform.classifier() + ", or set -Ddowsing-rod.binaryPath / DOWSING_ROD_BINARY.");
        }
        return new DowsingRod(extracted);
    }

    public Path executable() {
        return executable;
    }

    /** The version the executable reports, such as {@code 0.3.0}. */
    public String version() {
        ProcessResult result = run(Collections.singletonList("version"), null, null, null);
        String out = result.stdout().trim();
        if (result.exitCode() != 0 || !out.startsWith("dowsing-rod ")) {
            throw new DowsingRodException(executable + " is not a working dowsing-rod executable: "
                    + (out.isEmpty() ? result.errorMessage() : out));
        }
        return out.substring("dowsing-rod ".length()).trim();
    }

    /**
     * Scan {@code path} and return the parsed result. The output format is always JSON here;
     * any format in {@code options} is ignored.
     *
     * @throws DowsingRodException when the scan fails, for example on parse errors with
     *     {@code failOnError(true)}
     */
    public ScanResult scan(Path path, ScanOptions options) {
        List<String> args = new ArrayList<>(Arrays.asList("scan", path.toString()));
        args.addAll((options == null ? new ScanOptions() : options).withFormat("json").toArguments());
        ProcessResult result = run(args, null, null, null);
        if (result.exitCode() != 0) {
            throw new DowsingRodException("dowsing-rod scan failed: " + result.errorMessage());
        }
        return ScanResult.parse(result.stdout());
    }

    /**
     * Run the CLI with {@code args}. Output lines go to the consumers as they arrive; a stream
     * without a consumer is captured into the result instead.
     *
     * @param workingDirectory the process directory, or {@code null} for the current one
     */
    public ProcessResult run(List<String> args, Path workingDirectory, Consumer<String> stdout,
                             Consumer<String> stderr) {
        List<String> command = new ArrayList<>(args.size() + 1);
        command.add(executable.toAbsolutePath().toString());
        command.addAll(args);
        ProcessBuilder builder = new ProcessBuilder(command);
        if (workingDirectory != null) {
            builder.directory(workingDirectory.toFile());
        }
        Process process;
        try {
            process = builder.start();
        } catch (IOException e) {
            throw new DowsingRodException("cannot start " + executable + ": " + e.getMessage(), e);
        }
        try {
            process.getOutputStream().close();
            StreamPump out = new StreamPump(process.getInputStream(), stdout);
            StreamPump err = new StreamPump(process.getErrorStream(), stderr);
            Thread errThread = new Thread(err, "dowsing-rod-stderr");
            errThread.setDaemon(true);
            errThread.start();
            out.run();
            int exitCode = process.waitFor();
            errThread.join();
            return new ProcessResult(exitCode, out.captured(), err.captured());
        } catch (IOException e) {
            process.destroy();
            throw new DowsingRodException("cannot talk to " + executable + ": " + e.getMessage(), e);
        } catch (InterruptedException e) {
            process.destroy();
            Thread.currentThread().interrupt();
            throw new DowsingRodException("interrupted while running dowsing-rod", e);
        }
    }

    /** Reads one process stream line by line, forwarding or capturing it. */
    private static final class StreamPump implements Runnable {
        private final InputStream stream;
        private final Consumer<String> consumer;
        private final StringBuilder captured = new StringBuilder();
        private IOException failure;

        StreamPump(InputStream stream, Consumer<String> consumer) {
            this.stream = stream;
            this.consumer = consumer;
        }

        @Override
        public void run() {
            try (BufferedReader reader = new BufferedReader(new InputStreamReader(stream, StandardCharsets.UTF_8))) {
                String line;
                while ((line = reader.readLine()) != null) {
                    if (consumer != null) {
                        consumer.accept(line);
                    } else {
                        captured.append(line).append('\n');
                    }
                }
            } catch (IOException e) {
                failure = e;
            }
        }

        String captured() throws IOException {
            if (failure != null) {
                throw failure;
            }
            return captured.toString();
        }
    }
}
