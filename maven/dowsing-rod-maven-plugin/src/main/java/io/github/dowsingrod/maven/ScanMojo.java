package io.github.dowsingrod.maven;

import io.github.dowsingrod.DowsingRod;
import java.util.ArrayList;
import java.util.List;
import org.apache.maven.plugin.MojoExecutionException;
import org.apache.maven.plugin.MojoFailureException;
import org.apache.maven.plugins.annotations.LifecyclePhase;
import org.apache.maven.plugins.annotations.Mojo;

/**
 * Scan for structural refactoring opportunities: {@code dowsing-rod scan}. Options set here
 * override {@code dowsing-rod.toml}; options left unset come from it.
 *
 * <pre>mvn dowsing-rod:scan -Ddowsing-rod.ai=true -Ddowsing-rod.maxTokens=4000</pre>
 */
@Mojo(name = "scan", defaultPhase = LifecyclePhase.VERIFY, threadSafe = true, requiresProject = false)
public class ScanMojo extends AbstractScanMojo {
    @Override
    protected void run(DowsingRod rod) throws MojoExecutionException, MojoFailureException {
        List<String> args = new ArrayList<>();
        args.add("scan");
        args.add(target().toString());
        args.addAll(options().toArguments());
        Reports.produce(this, rod, args);
    }
}
