package io.github.dowsingrod.maven;

import io.github.dowsingrod.DowsingRod;
import io.github.dowsingrod.DowsingRodException;
import io.github.dowsingrod.ProcessResult;
import io.github.dowsingrod.ScanResult;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import org.apache.maven.plugin.MojoExecutionException;
import org.apache.maven.plugin.MojoFailureException;
import org.apache.maven.plugins.annotations.LifecyclePhase;
import org.apache.maven.plugins.annotations.Mojo;
import org.apache.maven.plugins.annotations.Parameter;

/**
 * Scan and fail the build when findings exceed thresholds. Every threshold is off unless set, so
 * without thresholds this goal only reports. The scan options are those of {@code scan}; the
 * report format is always JSON here ({@code outputFile} receives it), with a summary in the log.
 *
 * <pre>
 * &lt;configuration&gt;
 *   &lt;maxClustersFound&gt;0&lt;/maxClustersFound&gt;
 *   &lt;minRefactoringValue&gt;100&lt;/minRefactoringValue&gt;
 * &lt;/configuration&gt;
 * </pre>
 */
@Mojo(name = "check", defaultPhase = LifecyclePhase.VERIFY, threadSafe = true, requiresProject = false)
public class CheckMojo extends AbstractScanMojo {
    /** Fail when more clusters than this are reported (counting only those at or above minRefactoringValue). */
    @Parameter(property = "dowsing-rod.maxClustersFound")
    protected Integer maxClustersFound;

    /** Fail when the scan reports more high-value clusters than this. */
    @Parameter(property = "dowsing-rod.maxHighValueClusters")
    protected Integer maxHighValueClusters;

    /** Only clusters with at least this refactoring value count toward maxClustersFound and failOnClassifications. */
    @Parameter(property = "dowsing-rod.minRefactoringValue")
    protected Double minRefactoringValue;

    /** Fail when any counted cluster has one of these classifications, such as {@code duplicate}. */
    @Parameter(property = "dowsing-rod.failOnClassifications")
    protected List<String> failOnClassifications;

    /** How many clusters to list in the build log summary. */
    @Parameter(property = "dowsing-rod.summaryClusters", defaultValue = "10")
    protected int summaryClusters;

    @Override
    protected void run(DowsingRod rod) throws MojoExecutionException, MojoFailureException {
        Gate gate = new Gate(maxClustersFound, maxHighValueClusters, minRefactoringValue, failOnClassifications);
        List<String> args = new ArrayList<>();
        args.add("scan");
        args.add(target().toString());
        ProcessResult process = invoke(rod, withJson(args), null);
        ScanResult result;
        try {
            result = ScanResult.parse(process.stdout());
        } catch (DowsingRodException e) {
            throw new MojoExecutionException("Cannot read the dowsing-rod report: " + e.getMessage(), e);
        }
        Path output = outputPath();
        if (output != null) {
            write(output, process.stdout());
        }
        summarize(result);
        if (!gate.configured()) {
            getLog().info("No dowsing-rod thresholds configured; reporting only "
                    + "(set maxClustersFound, maxHighValueClusters, or failOnClassifications to gate the build).");
            return;
        }
        List<String> violations = gate.violations(result);
        if (!violations.isEmpty()) {
            throw new MojoFailureException("dowsing-rod check failed:\n  - " + String.join("\n  - ", violations));
        }
        getLog().info("dowsing-rod check passed.");
    }

    private List<String> withJson(List<String> args) throws MojoExecutionException {
        List<String> flags = options().toArguments();
        // The check always parses JSON, whatever format the POM or dowsing-rod.toml asks for.
        for (int i = 0; i < flags.size(); i++) {
            String flag = flags.get(i);
            if ("--ai".equals(flag)) {
                continue;
            }
            if ("--format".equals(flag)) {
                i++;
                continue;
            }
            args.add(flag);
        }
        args.add("--format");
        args.add("json");
        return args;
    }

    private void summarize(ScanResult result) {
        getLog().info(String.format(Locale.ROOT, "dowsing-rod %s: %d files, %d functions, %d clusters (%d high value)",
                result.toolVersion(), result.statistic("files_scanned"), result.statistic("functions_found"),
                result.clusters().size(), result.statistic("high_value_clusters")));
        int shown = 0;
        for (ScanResult.Cluster cluster : result.clusters()) {
            if (shown++ >= summaryClusters) {
                getLog().info("  ... " + (result.clusters().size() - summaryClusters) + " more (see outputFile or run dowsing-rod:scan)");
                break;
            }
            getLog().info(String.format(Locale.ROOT, "  %s  %s  %s  sim=%.0f%%  value=%.0f", cluster.id(),
                    cluster.language(), cluster.classification(), cluster.averageSimilarity() * 100,
                    cluster.refactoringValue()));
            for (String member : cluster.members()) {
                getLog().info("      " + member);
            }
        }
        if (!result.parseErrors().isEmpty()) {
            getLog().warn(result.parseErrors().size() + " file(s) could not be parsed; set failOnError=true to fail on them.");
        }
    }
}
