package io.github.dowsingrod.maven;

import io.github.dowsingrod.ScanResult;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Locale;

/** The thresholds of the {@code check} goal. Every threshold is off unless set. */
final class Gate {
    final Integer maxClustersFound;
    final Integer maxHighValueClusters;
    final Double minRefactoringValue;
    final List<String> failOnClassifications;

    Gate(Integer maxClustersFound, Integer maxHighValueClusters, Double minRefactoringValue,
         List<String> failOnClassifications) {
        this.maxClustersFound = maxClustersFound;
        this.maxHighValueClusters = maxHighValueClusters;
        this.minRefactoringValue = minRefactoringValue;
        List<String> classifications = new ArrayList<>();
        if (failOnClassifications != null) {
            for (String classification : failOnClassifications) {
                if (classification != null && !classification.trim().isEmpty()) {
                    classifications.add(classification.trim().toLowerCase(Locale.ROOT));
                }
            }
        }
        this.failOnClassifications = Collections.unmodifiableList(classifications);
    }

    boolean configured() {
        return maxClustersFound != null || maxHighValueClusters != null || !failOnClassifications.isEmpty();
    }

    /** Clusters that count toward the thresholds: those at or above minRefactoringValue. */
    List<ScanResult.Cluster> counted(ScanResult result) {
        List<ScanResult.Cluster> counted = new ArrayList<>();
        for (ScanResult.Cluster cluster : result.clusters()) {
            if (minRefactoringValue == null || cluster.refactoringValue() >= minRefactoringValue) {
                counted.add(cluster);
            }
        }
        return counted;
    }

    /** One message per violated threshold; empty when the build may pass. */
    List<String> violations(ScanResult result) {
        List<String> violations = new ArrayList<>();
        List<ScanResult.Cluster> counted = counted(result);
        String scope = minRefactoringValue == null ? "" : " with refactoring value >= " + minRefactoringValue;
        if (maxClustersFound != null && counted.size() > maxClustersFound) {
            violations.add(counted.size() + " clusters" + scope + " exceeds maxClustersFound=" + maxClustersFound);
        }
        long highValue = result.statistic("high_value_clusters");
        if (maxHighValueClusters != null && highValue > maxHighValueClusters) {
            violations.add(highValue + " high-value clusters exceeds maxHighValueClusters=" + maxHighValueClusters);
        }
        for (ScanResult.Cluster cluster : counted) {
            if (failOnClassifications.contains(cluster.classification())) {
                violations.add(cluster.id() + " is " + cluster.classification() + scope
                        + ", listed in failOnClassifications");
            }
        }
        return violations;
    }
}
