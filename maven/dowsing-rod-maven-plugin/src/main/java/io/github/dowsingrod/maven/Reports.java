package io.github.dowsingrod.maven;

import io.github.dowsingrod.DowsingRod;
import io.github.dowsingrod.ProcessResult;
import java.nio.file.Path;
import java.util.List;
import java.util.function.Consumer;
import org.apache.maven.plugin.MojoExecutionException;
import org.apache.maven.plugin.MojoFailureException;

/** Runs a report-producing command (scan / auto) and handles the optional output file. */
final class Reports {
    private Reports() {}

    static void produce(AbstractScanMojo mojo, DowsingRod rod, List<String> args)
            throws MojoExecutionException, MojoFailureException {
        Path output = mojo.outputPath();
        if (output == null) {
            mojo.invoke(rod, args, mojo.infoLog());
            return;
        }
        ProcessResult result = mojo.invoke(rod, args, null);
        String report = result.stdout();
        mojo.write(output, report);
        // Human and AI reports are also shown in the build log; JSON goes only to the file.
        String trimmed = report.trim();
        if (!(trimmed.startsWith("{") || trimmed.startsWith("["))) {
            Consumer<String> log = mojo.infoLog();
            for (String line : report.split("\\R", -1)) {
                log.accept(line);
            }
        }
    }
}
