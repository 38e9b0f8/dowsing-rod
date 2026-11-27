package io.github.dowsingrod.maven;

import io.github.dowsingrod.DowsingRod;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.apache.maven.plugin.MojoExecutionException;
import org.apache.maven.plugin.MojoFailureException;
import org.apache.maven.plugins.annotations.Mojo;
import org.apache.maven.plugins.annotations.Parameter;

/**
 * Write {@code dowsing-rod.toml} with the recommended defaults: {@code dowsing-rod init}. Refuses
 * to overwrite existing configuration unless {@code -Ddowsing-rod.force=true}.
 */
@Mojo(name = "init", threadSafe = true, requiresProject = false)
public class InitMojo extends AbstractDowsingRodMojo {
    /** Overwrite an existing configuration. */
    @Parameter(property = "dowsing-rod.force", defaultValue = "false")
    protected boolean force;

    @Override
    protected void run(DowsingRod rod) throws MojoExecutionException, MojoFailureException {
        List<String> args = new ArrayList<>(Arrays.asList("init", target().toString()));
        if (force) {
            args.add("--force");
        }
        invoke(rod, args, infoLog());
    }
}
