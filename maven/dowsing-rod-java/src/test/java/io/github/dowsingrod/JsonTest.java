package io.github.dowsingrod;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.util.Arrays;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;

class JsonTest {
    @Test
    @SuppressWarnings("unchecked")
    void parsesEveryValueKind() {
        Map<String, Object> map = (Map<String, Object>) Json.parse(
                " {\"a\": 1, \"b\": -2.5e1, \"c\": [true, false, null], \"d\": \"x\\\"\\n\\u00e9\\ud83d\\ude00\", \"e\": {}} ");
        assertEquals(1L, map.get("a"));
        assertEquals(-25.0, map.get("b"));
        assertEquals(Arrays.asList(true, false, null), map.get("c"));
        assertEquals("x\"\né😀", map.get("d"));
        assertEquals(0, ((Map<?, ?>) map.get("e")).size());
        assertEquals(Arrays.asList(), (List<Object>) Json.parse("[]"));
        assertNull(Json.parse("null"));
        assertEquals(1.0E20, Json.parse("100000000000000000000"));
    }

    @Test
    void rejectsMalformedInput() {
        for (String bad : new String[] {"", "{", "[1,", "{\"a\" 1}", "tru", "\"x", "1 2", "{\"a\":1,}"}) {
            assertThrows(DowsingRodException.class, () -> Json.parse(bad), bad);
        }
    }
}
