package io.github.dowsingrod;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;

/**
 * A parsed {@code dowsing-rod scan --format json} result. Typed accessors cover what build
 * tooling needs; {@link #raw()} exposes the whole document (see the README's Output Schema).
 */
public final class ScanResult {
    private final String json;
    private final Map<String, Object> root;

    private ScanResult(String json, Map<String, Object> root) {
        this.json = json;
        this.root = root;
    }

    /** Parse the CLI's JSON output. */
    @SuppressWarnings("unchecked")
    public static ScanResult parse(String json) {
        Object value = Json.parse(json);
        if (!(value instanceof Map)) {
            throw new DowsingRodException("dowsing-rod JSON output is not an object");
        }
        return new ScanResult(json, (Map<String, Object>) value);
    }

    /** The JSON text as the CLI printed it. */
    public String json() {
        return json;
    }

    /** The whole document as maps, lists, strings, numbers, and booleans. */
    public Map<String, Object> raw() {
        return Collections.unmodifiableMap(root);
    }

    public String schemaVersion() {
        return (String) root.get("schema_version");
    }

    public String toolVersion() {
        return (String) root.get("tool_version");
    }

    @SuppressWarnings("unchecked")
    public Map<String, Object> statistics() {
        Object value = root.get("statistics");
        return value instanceof Map ? (Map<String, Object>) value : Collections.<String, Object>emptyMap();
    }

    /** A numeric statistic such as {@code files_scanned} or {@code high_value_clusters}. */
    public long statistic(String name) {
        Object value = statistics().get(name);
        return value instanceof Number ? ((Number) value).longValue() : 0L;
    }

    @SuppressWarnings("unchecked")
    public List<Map<String, Object>> functions() {
        return list("functions");
    }

    @SuppressWarnings("unchecked")
    public List<Map<String, Object>> parseErrors() {
        return list("parse_errors");
    }

    /** Reported clusters, most valuable first, as the CLI ranked them. */
    public List<Cluster> clusters() {
        List<Cluster> clusters = new ArrayList<>();
        for (Map<String, Object> cluster : list("clusters")) {
            clusters.add(new Cluster(cluster, functions()));
        }
        return clusters;
    }

    @SuppressWarnings("unchecked")
    private List<Map<String, Object>> list(String key) {
        Object value = root.get(key);
        if (!(value instanceof List)) {
            return Collections.emptyList();
        }
        List<Map<String, Object>> items = new ArrayList<>();
        for (Object item : (List<Object>) value) {
            if (item instanceof Map) {
                items.add((Map<String, Object>) item);
            }
        }
        return items;
    }

    /** One refactoring opportunity. */
    public static final class Cluster {
        private final Map<String, Object> data;
        private final List<Map<String, Object>> functions;

        Cluster(Map<String, Object> data, List<Map<String, Object>> functions) {
            this.data = data;
            this.functions = functions;
        }

        public String id() {
            return String.valueOf(data.get("id"));
        }

        /** Such as {@code strategy_candidate}, {@code duplicate}, or {@code near_duplicate}. */
        public String classification() {
            return String.valueOf(data.get("classification"));
        }

        public double refactoringValue() {
            return number("refactoring_value");
        }

        public double averageSimilarity() {
            return number("average_similarity");
        }

        public double confidence() {
            return number("confidence");
        }

        public String reason() {
            Object reason = data.get("reason");
            return reason == null ? "" : reason.toString();
        }

        /** Members as {@code file:qualified_name:start-end}. */
        @SuppressWarnings("unchecked")
        public List<String> members() {
            List<String> members = new ArrayList<>();
            Object indices = data.get("function_indices");
            if (!(indices instanceof List)) {
                return members;
            }
            for (Object index : (List<Object>) indices) {
                int i = ((Number) index).intValue();
                if (i < 0 || i >= functions.size()) {
                    continue;
                }
                Map<String, Object> function = functions.get(i);
                members.add(function.get("file") + ":" + function.get("qualified_name") + ":"
                        + function.get("start_line") + "-" + function.get("end_line"));
            }
            return members;
        }

        /** The language of the cluster's functions (clusters never mix languages). */
        @SuppressWarnings("unchecked")
        public String language() {
            Object indices = data.get("function_indices");
            if (indices instanceof List && !((List<Object>) indices).isEmpty()) {
                int i = ((Number) ((List<Object>) indices).get(0)).intValue();
                if (i >= 0 && i < functions.size()) {
                    return String.valueOf(functions.get(i).get("language"));
                }
            }
            return "";
        }

        public Map<String, Object> raw() {
            return Collections.unmodifiableMap(data);
        }

        private double number(String key) {
            Object value = data.get(key);
            return value instanceof Number ? ((Number) value).doubleValue() : 0.0;
        }
    }
}
