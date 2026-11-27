package io.github.dowsingrod;

/** A failure to locate, verify, or run the native {@code dowsing-rod} executable. */
public class DowsingRodException extends RuntimeException {
    private static final long serialVersionUID = 1L;

    public DowsingRodException(String message) {
        super(message);
    }

    public DowsingRodException(String message, Throwable cause) {
        super(message, cause);
    }
}
