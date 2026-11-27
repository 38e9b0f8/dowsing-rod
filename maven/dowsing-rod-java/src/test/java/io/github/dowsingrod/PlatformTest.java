package io.github.dowsingrod;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import org.junit.jupiter.api.Test;

class PlatformTest {
    @Test
    void mapsJvmPropertiesToClassifiers() {
        assertEquals("linux-x86_64", Platform.detect("Linux", "amd64", false).classifier());
        assertEquals("linux-aarch_64", Platform.detect("Linux", "aarch64", false).classifier());
        assertEquals("linux-x86_64-musl", Platform.detect("Linux", "amd64", true).classifier());
        assertEquals("linux-aarch_64-musl", Platform.detect("Linux", "aarch64", true).classifier());
        assertEquals("osx-x86_64", Platform.detect("Mac OS X", "x86_64", false).classifier());
        assertEquals("osx-aarch_64", Platform.detect("Mac OS X", "aarch64", false).classifier());
        assertEquals("windows-x86_64", Platform.detect("Windows 11", "amd64", false).classifier());
    }

    @Test
    void muslOnlyAppliesToLinux() {
        assertEquals("osx-aarch_64", Platform.detect("Mac OS X", "aarch64", true).classifier());
    }

    @Test
    void executableNamesAndResourcePaths() {
        Platform windows = Platform.detect("Windows Server 2022", "amd64", false);
        assertTrue(windows.isWindows());
        assertEquals("dowsing-rod.exe", windows.executableName());
        assertEquals("native/windows-x86_64/dowsing-rod.exe", windows.resourcePath());
        Platform linux = Platform.detect("Linux", "amd64", false);
        assertFalse(linux.isWindows());
        assertEquals("native/linux-x86_64/dowsing-rod", linux.resourcePath());
    }

    @Test
    void unsupportedPlatformsExplainTheEscapeHatch() {
        DowsingRodException windowsArm = assertThrows(DowsingRodException.class,
                () -> Platform.detect("Windows 11", "aarch64", false));
        assertTrue(windowsArm.getMessage().contains("binaryPath"), windowsArm.getMessage());
        assertThrows(DowsingRodException.class, () -> Platform.detect("FreeBSD", "amd64", false));
        assertThrows(DowsingRodException.class, () -> Platform.detect("Linux", "ppc64le", false));
    }

    @Test
    void currentPlatformIsDetectedOnEveryCiRunner() {
        assertTrue(Platform.CLASSIFIERS.contains(Platform.current().classifier()));
    }
}
