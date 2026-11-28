package io.github.dowsingrod.maven;

import java.nio.charset.Charset;
import java.nio.charset.CharsetEncoder;
import java.util.HashMap;
import java.util.Map;

/**
 * Keeps the report readable on consoles that cannot show its Unicode symbols.
 *
 * <p>The CLI draws its report with box-drawing characters, arrows, and a few symbols. Maven writes
 * the log in the console's encoding, which on Windows is usually a legacy code page (437, 850,
 * 1252): characters it lacks become {@code ?}. Each such character is replaced by an ASCII
 * equivalent instead. Characters the console can show are left alone, and report files are
 * always written as UTF-8.
 */
final class ConsoleText {

    private static final Map<Character, String> ASCII = new HashMap<>();

    static {
        for (char c : "─━".toCharArray()) {
            ASCII.put(c, "-");
        }
        for (char c : "│┃".toCharArray()) {
            ASCII.put(c, "|");
        }
        for (char c : "┌┐└┘├┤┬┴┼".toCharArray()) {
            ASCII.put(c, "+");
        }
        ASCII.put('•', "*");
        ASCII.put('→', "->");
        ASCII.put('↔', "<->");
        ASCII.put('≈', "~");
        ASCII.put('—', "-");
        ASCII.put('…', "...");
        ASCII.put('✓', "+");
        ASCII.put('⚠', "!");
        ASCII.put('⛏', "*");
    }

    private final CharsetEncoder encoder;

    ConsoleText(Charset console) {
        this.encoder = console.newEncoder();
    }

    /** For the encoding Maven's console output uses in this JVM. */
    static ConsoleText forStandardOutput() {
        return new ConsoleText(standardOutputCharset());
    }

    /** {@code line}, with every character the console cannot show replaced by ASCII. */
    String apply(String line) {
        StringBuilder out = null;
        for (int i = 0; i < line.length(); i++) {
            char c = line.charAt(i);
            String replacement = c < 0x80 ? null : ASCII.get(c);
            if (replacement != null && !encoder.canEncode(c)) {
                if (out == null) {
                    out = new StringBuilder(line.length()).append(line, 0, i);
                }
                out.append(replacement);
            } else if (out != null) {
                out.append(c);
            }
        }
        return out == null ? line : out.toString();
    }

    /**
     * The charset {@code System.out} encodes with: {@code stdout.encoding} on Java 18+, {@code
     * sun.stdout.encoding} on older JDKs attached to a console, else the default charset.
     */
    static Charset standardOutputCharset() {
        for (String property : new String[] {"stdout.encoding", "sun.stdout.encoding"}) {
            String name = System.getProperty(property);
            if (name != null) {
                try {
                    return Charset.forName(name);
                } catch (RuntimeException e) {
                    // Unknown or unsupported name: try the next source.
                }
            }
        }
        return Charset.defaultCharset();
    }
}
