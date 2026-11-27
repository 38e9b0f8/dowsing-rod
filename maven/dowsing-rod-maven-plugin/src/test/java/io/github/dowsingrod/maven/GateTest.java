package io.github.dowsingrod.maven;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertTrue;

import io.github.dowsingrod.ScanResult;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import org.junit.jupiter.api.Test;

class GateTest {
    private static final ScanResult RESULT = ScanResult.parse("{"
            + "\"schema_version\":\"1.1\",\"tool_version\":\"0.0.0\","
            + "\"statistics\":{\"files_scanned\":2,\"high_value_clusters\":1},"
            + "\"functions\":[{\"file\":\"A.java\",\"qualified_name\":\"A::a\",\"start_line\":1,\"end_line\":3,\"language\":\"java\"},"
            + "{\"file\":\"B.java\",\"qualified_name\":\"B::b\",\"start_line\":4,\"end_line\":9,\"language\":\"java\"}],"
            + "\"clusters\":["
            + "{\"id\":\"C1\",\"classification\":\"strategy_candidate\",\"refactoring_value\":312.5,\"function_indices\":[0,1]},"
            + "{\"id\":\"C2\",\"classification\":\"duplicate\",\"refactoring_value\":12.0,\"function_indices\":[1,0]}],"
            + "\"parse_errors\":[]}");

    private static Gate gate(Integer clusters, Integer highValue, Double minValue, List<String> classifications) {
        return new Gate(clusters, highValue, minValue, classifications);
    }

    @Test
    void unsetGateIsReportOnly() {
        Gate gate = gate(null, null, null, null);
        assertFalse(gate.configured());
        assertTrue(gate.violations(RESULT).isEmpty());
        assertFalse(gate(null, null, 5.0, Collections.<String>emptyList()).configured(),
                "minRefactoringValue alone is a filter, not a threshold");
    }

    @Test
    void clusterCountRespectsMinimumValue() {
        assertEquals(1, gate(1, null, null, null).violations(RESULT).size());
        assertTrue(gate(2, null, null, null).violations(RESULT).isEmpty());
        assertTrue(gate(1, null, 100.0, null).violations(RESULT).isEmpty());
        assertEquals(1, gate(0, null, 100.0, null).violations(RESULT).size());
    }

    @Test
    void highValueThresholdUsesTheScannerStatistic() {
        assertTrue(gate(null, 1, null, null).violations(RESULT).isEmpty());
        assertEquals("1 high-value clusters exceeds maxHighValueClusters=0",
                gate(null, 0, null, null).violations(RESULT).get(0));
    }

    @Test
    void classificationsAreCaseInsensitiveAndFiltered() {
        List<String> violations = gate(null, null, null, Arrays.asList(" Duplicate ", "")).violations(RESULT);
        assertEquals(1, violations.size());
        assertTrue(violations.get(0).startsWith("C2 is duplicate"));
        assertTrue(gate(null, null, 50.0, Collections.singletonList("duplicate")).violations(RESULT).isEmpty());
    }

    @Test
    void clusterMembersAndLanguageComeFromFunctions() {
        ScanResult.Cluster cluster = RESULT.clusters().get(0);
        assertEquals(Arrays.asList("A.java:A::a:1-3", "B.java:B::b:4-9"), cluster.members());
        assertEquals("java", cluster.language());
    }
}
