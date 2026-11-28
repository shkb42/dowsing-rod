package io.github.dowsingrod.maven;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertSame;

import java.nio.charset.Charset;
import java.nio.charset.StandardCharsets;
import org.junit.jupiter.api.Test;

class ConsoleTextTest {
    private static final String REPORT_LINES = "  ⛏  Dowsing Rod\n"
            + "  ├── Files scanned:   50\n"
            + "  ┌──────┬─────┐\n"
            + "    • A.java:a:1-3\n"
            + "    Common pipeline: a → b ≈ c ↔ d…\n"
            + "    Very high similarity — likely copy-pasted code. ✓ ⚠";

    @Test
    void utf8ConsoleKeepsEverySymbol() {
        ConsoleText console = new ConsoleText(StandardCharsets.UTF_8);
        assertSame(REPORT_LINES, console.apply(REPORT_LINES));
    }

    @Test
    void asciiConsoleGetsAsciiEquivalents() {
        assertEquals("  *  Dowsing Rod\n"
                        + "  +-- Files scanned:   50\n"
                        + "  +------+-----+\n"
                        + "    * A.java:a:1-3\n"
                        + "    Common pipeline: a -> b ~ c <-> d...\n"
                        + "    Very high similarity - likely copy-pasted code. + !",
                new ConsoleText(StandardCharsets.US_ASCII).apply(REPORT_LINES));
    }

    @Test
    void windowsCodePageKeepsWhatItCanShow() {
        // Code page 850 has box-drawing characters but no bullet, arrows, or dashes:
        // exactly the mix seen on a default Windows console.
        ConsoleText console = new ConsoleText(Charset.forName("IBM850"));
        assertEquals("  ├── Files scanned:   50", console.apply("  ├── Files scanned:   50"));
        assertEquals("    * A.java -> B.java - done", console.apply("    • A.java → B.java — done"));
    }

    @Test
    void otherCharactersAreLeftAlone() {
        String path = "src/main/java/Überprüfung/名前.java";
        assertSame(path, new ConsoleText(StandardCharsets.US_ASCII).apply(path));
    }
}
