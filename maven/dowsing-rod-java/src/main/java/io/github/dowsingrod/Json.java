package io.github.dowsingrod;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * Minimal JSON reader for the scanner's output, so the library has no dependencies.
 * Objects become {@code LinkedHashMap<String, Object>}, arrays {@code ArrayList<Object>},
 * integral numbers {@code Long}, other numbers {@code Double}.
 */
final class Json {
    private final String text;
    private int index;

    private Json(String text) {
        this.text = text;
    }

    static Object parse(String text) {
        Json json = new Json(text);
        json.whitespace();
        Object value = json.value();
        json.whitespace();
        if (json.index != text.length()) {
            throw json.error("trailing characters");
        }
        return value;
    }

    private Object value() {
        if (index >= text.length()) {
            throw error("unexpected end of input");
        }
        char c = text.charAt(index);
        switch (c) {
            case '{':
                return object();
            case '[':
                return array();
            case '"':
                return string();
            case 't':
                return literal("true", Boolean.TRUE);
            case 'f':
                return literal("false", Boolean.FALSE);
            case 'n':
                return literal("null", null);
            default:
                if (c == '-' || (c >= '0' && c <= '9')) {
                    return number();
                }
                throw error("unexpected character '" + c + "'");
        }
    }

    private Map<String, Object> object() {
        Map<String, Object> map = new LinkedHashMap<>();
        index++;
        whitespace();
        if (peek('}')) {
            index++;
            return map;
        }
        while (true) {
            whitespace();
            if (!peek('"')) {
                throw error("expected a key");
            }
            String key = string();
            whitespace();
            expect(':');
            whitespace();
            map.put(key, value());
            whitespace();
            if (peek(',')) {
                index++;
            } else {
                expect('}');
                return map;
            }
        }
    }

    private List<Object> array() {
        List<Object> list = new ArrayList<>();
        index++;
        whitespace();
        if (peek(']')) {
            index++;
            return list;
        }
        while (true) {
            whitespace();
            list.add(value());
            whitespace();
            if (peek(',')) {
                index++;
            } else {
                expect(']');
                return list;
            }
        }
    }

    private String string() {
        index++;
        StringBuilder out = new StringBuilder();
        while (true) {
            if (index >= text.length()) {
                throw error("unterminated string");
            }
            char c = text.charAt(index++);
            if (c == '"') {
                return out.toString();
            }
            if (c != '\\') {
                out.append(c);
                continue;
            }
            if (index >= text.length()) {
                throw error("unterminated escape");
            }
            char e = text.charAt(index++);
            switch (e) {
                case '"':
                case '\\':
                case '/':
                    out.append(e);
                    break;
                case 'b':
                    out.append('\b');
                    break;
                case 'f':
                    out.append('\f');
                    break;
                case 'n':
                    out.append('\n');
                    break;
                case 'r':
                    out.append('\r');
                    break;
                case 't':
                    out.append('\t');
                    break;
                case 'u':
                    if (index + 4 > text.length()) {
                        throw error("bad unicode escape");
                    }
                    try {
                        out.append((char) Integer.parseInt(text.substring(index, index + 4), 16));
                    } catch (NumberFormatException ex) {
                        throw error("bad unicode escape");
                    }
                    index += 4;
                    break;
                default:
                    throw error("bad escape \\" + e);
            }
        }
    }

    private Object number() {
        int start = index;
        if (peek('-')) {
            index++;
        }
        boolean integral = true;
        while (index < text.length()) {
            char c = text.charAt(index);
            if (c >= '0' && c <= '9') {
                index++;
            } else if (c == '.' || c == 'e' || c == 'E' || c == '+' || c == '-') {
                integral = false;
                index++;
            } else {
                break;
            }
        }
        String raw = text.substring(start, index);
        try {
            if (integral) {
                try {
                    return Long.parseLong(raw);
                } catch (NumberFormatException overflow) {
                    return Double.parseDouble(raw);
                }
            }
            return Double.parseDouble(raw);
        } catch (NumberFormatException ex) {
            throw error("bad number " + raw);
        }
    }

    private Object literal(String word, Object value) {
        if (!text.startsWith(word, index)) {
            throw error("expected " + word);
        }
        index += word.length();
        return value;
    }

    private void whitespace() {
        while (index < text.length()) {
            char c = text.charAt(index);
            if (c == ' ' || c == '\n' || c == '\r' || c == '\t') {
                index++;
            } else {
                return;
            }
        }
    }

    private boolean peek(char c) {
        return index < text.length() && text.charAt(index) == c;
    }

    private void expect(char c) {
        if (!peek(c)) {
            throw error("expected '" + c + "'");
        }
        index++;
    }

    private DowsingRodException error(String message) {
        return new DowsingRodException("invalid dowsing-rod JSON at offset " + index + ": " + message);
    }
}
