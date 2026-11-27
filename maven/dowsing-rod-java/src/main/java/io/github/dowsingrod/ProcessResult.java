package io.github.dowsingrod;

/** Exit code and captured output of one {@code dowsing-rod} invocation. */
public final class ProcessResult {
    private final int exitCode;
    private final String stdout;
    private final String stderr;

    ProcessResult(int exitCode, String stdout, String stderr) {
        this.exitCode = exitCode;
        this.stdout = stdout;
        this.stderr = stderr;
    }

    /** 0 on success, 1 when the scan failed (for example parse errors with fail-on-error), 2 for usage errors. */
    public int exitCode() {
        return exitCode;
    }

    /** Captured standard output; empty when a line consumer was given for it. */
    public String stdout() {
        return stdout;
    }

    /** Captured standard error; empty when a line consumer was given for it. */
    public String stderr() {
        return stderr;
    }

    /** The CLI's error message (its last {@code error:} line), or the last stderr line. */
    public String errorMessage() {
        String message = null;
        for (String line : stderr.split("\\R")) {
            if (line.startsWith("error:")) {
                message = line.substring("error:".length()).trim();
            } else if (message == null && !line.trim().isEmpty()) {
                message = line.trim();
            }
        }
        return message == null ? "exit code " + exitCode : message;
    }
}
