package payload;

// Source of the base64 blob in fixtures/src/blob-embedded-code and corr-hidden-loader.
// It is never loaded: the fixtures only carry it as a string. Regenerate with
// `javac --release 17 -d <dir> Payload.java` and base64-encode Payload.class.
public final class Payload {
    void unused() throws Exception {
        Runtime.getRuntime().exec(new String[] {"echo", "fixture"});
    }
}
