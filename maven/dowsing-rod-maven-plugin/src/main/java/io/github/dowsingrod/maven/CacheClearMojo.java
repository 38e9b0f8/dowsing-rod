package io.github.dowsingrod.maven;

import io.github.dowsingrod.DowsingRod;
import java.util.Arrays;
import org.apache.maven.plugin.MojoExecutionException;
import org.apache.maven.plugin.MojoFailureException;
import org.apache.maven.plugins.annotations.Mojo;

/** Delete the {@code .dowsing-rod-cache/} analysis cache: {@code dowsing-rod cache clear}. */
@Mojo(name = "cache-clear", threadSafe = true, requiresProject = false)
public class CacheClearMojo extends AbstractDowsingRodMojo {
    @Override
    protected void run(DowsingRod rod) throws MojoExecutionException, MojoFailureException {
        invoke(rod, Arrays.asList("cache", "clear", target().toString()), infoLog());
    }
}
