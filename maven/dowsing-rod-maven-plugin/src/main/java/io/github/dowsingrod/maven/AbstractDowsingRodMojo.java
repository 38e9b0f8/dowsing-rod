package io.github.dowsingrod.maven;

import io.github.dowsingrod.DowsingRod;
import io.github.dowsingrod.DowsingRodException;
import io.github.dowsingrod.NativeBinary;
import io.github.dowsingrod.Platform;
import io.github.dowsingrod.ProcessResult;
import io.github.dowsingrod.Version;
import java.io.File;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.Consumer;
import org.apache.maven.execution.MavenSession;
import org.apache.maven.model.Plugin;
import org.apache.maven.plugin.AbstractMojo;
import org.apache.maven.plugin.MojoExecutionException;
import org.apache.maven.plugin.MojoFailureException;
import org.apache.maven.plugin.descriptor.PluginDescriptor;
import org.apache.maven.plugins.annotations.Component;
import org.apache.maven.plugins.annotations.Parameter;
import org.apache.maven.project.MavenProject;
import org.eclipse.aether.RepositorySystem;
import org.eclipse.aether.artifact.DefaultArtifact;
import org.eclipse.aether.repository.RemoteRepository;
import org.eclipse.aether.resolution.ArtifactRequest;
import org.eclipse.aether.resolution.ArtifactResolutionException;
import org.eclipse.aether.resolution.ArtifactResult;

/** Shared behaviour: where to run in a reactor, which binary to run, and how to report. */
abstract class AbstractDowsingRodMojo extends AbstractMojo {
    /** Versions already checked in this JVM, so `mvn -T` and multi-goal builds check once. */
    private static final Map<Path, String> CHECKED_VERSIONS = new ConcurrentHashMap<>();

    @Parameter(defaultValue = "${session}", readonly = true, required = true)
    protected MavenSession session;

    @Parameter(defaultValue = "${project}", readonly = true, required = true)
    protected MavenProject project;

    @Parameter(defaultValue = "${plugin}", readonly = true, required = true)
    protected PluginDescriptor plugin;

    @Component
    protected RepositorySystem repositorySystem;

    /** Skip this goal. */
    @Parameter(property = "dowsing-rod.skip", defaultValue = "false")
    protected boolean skip;

    /**
     * Run a dowsing-rod executable you provide instead of the prebuilt one from
     * {@code dowsing-rod-native}: an unsupported platform, a binary built from source, or an
     * internally rebuilt copy.
     */
    @Parameter(property = "dowsing-rod.binaryPath")
    protected File binaryPath;

    /**
     * Run in every module, scanning each module's own folder. By default the goal runs once, at
     * the reactor root, and scans the whole project, so duplication across modules is found.
     */
    @Parameter(property = "dowsing-rod.perModule", defaultValue = "false")
    protected boolean perModule;

    /** What to scan. Defaults to the folder of the project the goal runs in. */
    @Parameter(property = "dowsing-rod.path")
    protected File path;

    @Override
    public final void execute() throws MojoExecutionException, MojoFailureException {
        if (skip) {
            getLog().info("Skipping dowsing-rod (dowsing-rod.skip=true).");
            return;
        }
        if (!shouldRunHere()) {
            getLog().info("dowsing-rod runs once, at the reactor root (" + executionRoot().getArtifactId()
                    + "), which scans this module too; set perModule=true to run per module.");
            return;
        }
        run(executable());
    }

    protected abstract void run(DowsingRod rod) throws MojoExecutionException, MojoFailureException;

    /** The folder this goal scans or configures. */
    protected Path target() {
        File folder = path != null ? path : project.getBasedir();
        if (folder == null) {
            folder = new File(session.getExecutionRootDirectory());
        }
        return folder.toPath().toAbsolutePath().normalize();
    }

    // ─── Reactor placement ───────────────────────────────────────────────

    private boolean shouldRunHere() {
        if (perModule) {
            return true;
        }
        MavenProject root = executionRoot();
        if (root == null || sameProject(root, project)) {
            return true;
        }
        // The root covers this module only when the goal runs there too: either it was named on
        // the command line (so it runs in every module, root first) or the root's build uses the
        // plugin. Otherwise this module declared the plugin itself, and runs for its own folder.
        return !(invokedFromCommandLine() || declaresThisPlugin(root));
    }

    /** The top-level project when it is part of this build, otherwise the first project built. */
    private MavenProject executionRoot() {
        MavenProject top = session.getTopLevelProject();
        List<MavenProject> projects = session.getProjects();
        if (top != null) {
            for (MavenProject candidate : projects) {
                if (sameProject(candidate, top)) {
                    return candidate;
                }
            }
        }
        return projects.isEmpty() ? null : projects.get(0);
    }

    private static boolean sameProject(MavenProject a, MavenProject b) {
        return a == b || (a.getFile() != null && a.getFile().equals(b.getFile())
                && a.getArtifactId().equals(b.getArtifactId()));
    }

    private boolean invokedFromCommandLine() {
        String prefix = plugin.getGoalPrefix() + ":";
        String coordinates = plugin.getGroupId() + ":" + plugin.getArtifactId() + ":";
        for (String goal : session.getGoals()) {
            if (goal.startsWith(prefix) || goal.startsWith(coordinates)) {
                return true;
            }
        }
        return false;
    }

    private boolean declaresThisPlugin(MavenProject root) {
        for (Plugin candidate : root.getBuildPlugins()) {
            if (plugin.getArtifactId().equals(candidate.getArtifactId())
                    && plugin.getGroupId().equals(candidate.getGroupId())) {
                return true;
            }
        }
        return false;
    }

    // ─── Binary resolution ───────────────────────────────────────────────

    private DowsingRod executable() throws MojoExecutionException {
        String expected = Version.get();
        if (binaryPath != null) {
            if (!binaryPath.isFile()) {
                throw new MojoExecutionException("binaryPath " + binaryPath + " does not exist or is not a file.");
            }
            DowsingRod rod = new DowsingRod(binaryPath.toPath());
            String actual = checkedVersion(rod);
            if (!expected.equals(actual)) {
                getLog().warn("binaryPath " + binaryPath + " is dowsing-rod " + actual + "; this plugin is "
                        + expected + ". Output may differ from the documented behaviour.");
            }
            return rod;
        }
        try {
            Platform platform = Platform.current();
            Path jar = resolveNativeJar(platform, expected);
            DowsingRod rod = new DowsingRod(extract(jar, platform));
            String actual = checkedVersion(rod);
            if (!expected.equals(actual)) {
                throw new MojoExecutionException("dowsing-rod-native " + platform.classifier() + " reports version "
                        + actual + " but the plugin is " + expected + "; the local repository may be corrupt.");
            }
            return rod;
        } catch (DowsingRodException e) {
            throw new MojoExecutionException(e.getMessage(), e);
        }
    }

    private Path resolveNativeJar(Platform platform, String version) throws MojoExecutionException {
        DefaultArtifact artifact = new DefaultArtifact(Version.GROUP_ID, Version.NATIVE_ARTIFACT_ID,
                platform.classifier(), "jar", version);
        List<RemoteRepository> repositories = new ArrayList<>(project.getRemotePluginRepositories());
        for (RemoteRepository repository : project.getRemoteProjectRepositories()) {
            if (!repositories.contains(repository)) {
                repositories.add(repository);
            }
        }
        try {
            ArtifactResult result = repositorySystem.resolveArtifact(session.getRepositorySession(),
                    new ArtifactRequest(artifact, repositories, null));
            File file = result.getArtifact().getFile();
            getLog().debug("dowsing-rod native binary: " + file);
            return file.toPath();
        } catch (ArtifactResolutionException e) {
            throw new MojoExecutionException("Cannot resolve " + artifact + " for this platform ("
                    + platform.classifier() + "). Check repository access, or set binaryPath. " + e.getMessage(), e);
        }
    }

    /**
     * Extract next to the native jar in the local repository, so it is shared by every project and
     * survives `mvn clean`. A read-only local repository falls back to this project's build folder.
     */
    private Path extract(Path jar, Platform platform) {
        Path beside = jar.getParent().resolve("extracted");
        try {
            return NativeBinary.extractFromJar(jar, platform, beside);
        } catch (DowsingRodException e) {
            if (!(e.getCause() instanceof IOException) || e.getMessage().contains("SHA-256")) {
                throw e;
            }
            Path fallback = new File(project.getBuild().getDirectory(), "dowsing-rod").toPath();
            getLog().debug("Cannot extract into " + beside + " (" + e.getMessage() + "); using " + fallback);
            return NativeBinary.extractFromJar(jar, platform, fallback);
        }
    }

    private static String checkedVersion(DowsingRod rod) {
        return CHECKED_VERSIONS.computeIfAbsent(rod.executable().toAbsolutePath(), key -> rod.version());
    }

    // ─── Running and reporting ───────────────────────────────────────────

    /** Run the CLI in {@link #target()}, streaming stderr to the log, and map its exit code. */
    protected ProcessResult invoke(DowsingRod rod, List<String> args, Consumer<String> stdout)
            throws MojoExecutionException, MojoFailureException {
        getLog().debug("dowsing-rod " + String.join(" ", args));
        StringBuilder errors = new StringBuilder();
        ProcessResult result;
        try {
            Path directory = target();
            result = rod.run(args, Files.isDirectory(directory) ? directory : directory.getParent(), stdout, line -> {
                errors.append(line).append('\n');
                if (line.startsWith("warning:")) {
                    getLog().warn(line.substring("warning:".length()).trim());
                } else if (!line.startsWith("error:")) {
                    getLog().warn(line);
                }
            });
        } catch (DowsingRodException e) {
            throw new MojoExecutionException(e.getMessage(), e);
        }
        String message = errorMessage(errors.toString());
        switch (result.exitCode()) {
            case 0:
                return result;
            case 2:
                // clap usage error: a plugin/binary mismatch rather than a project problem.
                throw new MojoExecutionException("dowsing-rod rejected its arguments: " + message);
            default:
                throw new MojoFailureException("dowsing-rod failed: " + message);
        }
    }

    /** All `error:` text the CLI printed, including the parse-error list that follows it. */
    private static String errorMessage(String stderr) {
        int start = stderr.indexOf("error:");
        String message = (start < 0 ? stderr : stderr.substring(start + "error:".length())).trim();
        return message.isEmpty() ? "no error message" : message;
    }

    /** Log every line at info level, as Maven users expect for a report. */
    protected Consumer<String> infoLog() {
        return line -> getLog().info(line);
    }
}
