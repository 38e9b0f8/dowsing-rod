package io.github.dowsingrod.maven;

import io.github.dowsingrod.DowsingRod;
import java.util.Arrays;
import org.apache.maven.plugin.MojoExecutionException;
import org.apache.maven.plugin.MojoFailureException;
import org.apache.maven.plugins.annotations.LifecyclePhase;
import org.apache.maven.plugins.annotations.Mojo;

/**
 * Scan using only the project's {@code dowsing-rod.toml}: {@code dowsing-rod auto}. Fails when no
 * configuration exists in the project folder or any folder above it; run
 * {@code mvn dowsing-rod:init} first. Scan options in the POM are ignored, by design.
 */
@Mojo(name = "auto", defaultPhase = LifecyclePhase.VERIFY, threadSafe = true, requiresProject = false)
public class AutoMojo extends AbstractScanMojo {
    @Override
    protected void run(DowsingRod rod) throws MojoExecutionException, MojoFailureException {
        Reports.produce(this, rod, Arrays.asList("auto", target().toString()));
    }
}
