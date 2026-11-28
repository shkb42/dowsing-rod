# Security policy

## Reporting a vulnerability

Please report vulnerabilities privately through GitHub: **Security → Report a vulnerability** on
[38e9b0f8/dowsing-rod](https://github.com/38e9b0f8/dowsing-rod/security/advisories/new). Do not open a
public issue.

## Supported versions

Only the latest release receives fixes.

## Verifying Maven artifacts

The Maven artifacts are published from the [shkb42/dowsing-rod](https://github.com/shkb42/dowsing-rod) fork.
Every artifact on Maven Central under `io.github.shkb42` is GPG-signed (`.asc`) and comes with a
CycloneDX SBOM (`-cyclonedx.json`). The jars also carry a GitHub build-provenance attestation that ties
them to the workflow run that built them:

```bash
gh attestation verify dowsing-rod-maven-plugin-<version>.jar -R shkb42/dowsing-rod
```

At run time, `dowsing-rod-java` checks the SHA-256 of the native binary before executing it.
