# TODO: Java and Maven release

The work on branch `feature/java-maven` (Java analysis, musl builds, Maven distribution) still has to be merged and published. The design, impacts, and decisions are in [docs/java-maven-action-plan.md](docs/java-maven-action-plan.md).

## 1. Decide the Maven namespace (blocker)

- [ ] Confirm who publishes. Maven Central only accepts `io.github.<name>` from whoever controls that GitHub account.
  - The upstream owner (`38e9b0f8`) publishes: keep `io.github.38e9b0f8`, and open the PRs upstream.
  - This fork publishes on its own: switch the `groupId` to `io.github.<your-account>`. That covers `maven/**/pom.xml`, `maven/release-smoke/pom.xml`, `Version.GROUP_ID` in `dowsing-rod-java`, the `scm`/`url` entries in `maven/pom.xml`, `maven-release.yml` (staging path check), the README, `maven/README.md`, `llms.txt`, `docs/platform-support.md`, and the CHANGELOG.

## 2. Review and merge

- [ ] Split into three PRs, each based on the one before:
  1. **Java language support:** `crates/`, `Cargo.*`, `tests/`, `THIRD_PARTY_NOTICES.md`, and the language docs. It can be released on its own through PyPI, npm, and NuGet.
  2. **musl/Alpine builds:** `npm/`, `nuget/`, `build-native.yml`, and the release checks.
  3. **Maven distribution:** `maven/`, `scripts/package-maven.mjs`, `scripts/test-maven-release.mjs`, `scripts/version.mjs`, `ci.yml`, `maven-release.yml`, the toolchain action, and the Maven docs.
- [ ] Optional: cut the IT fixture copies in `maven/dowsing-rod-maven-plugin/src/it` by sharing one fixture folder (about 50 fewer files).
- [ ] Commit `maven/mvnw` as executable: `git add --chmod=+x maven/mvnw`. CI calls it with `sh mvnw`, so this only matters for local `./mvnw`.
- [ ] Push and wait for CI to pass. These jobs have not run on GitHub yet:
  - [ ] `maven` on Linux (JDK 8 and 21), Windows, and macOS
  - [ ] `musl`: static binary, npm launcher, and the Maven build on Alpine
  - [ ] Existing jobs: Rust and Python, glibc 2.17, npm and NuGet packed installs
- [ ] Merge to `main`.

## 3. One-time Maven Central setup

- [ ] Sign in at <https://central.sonatype.com> with GitHub and check that the namespace shows as verified.
- [ ] Generate a user token (*Account → Generate User Token*).
- [ ] Create a GPG key and publish it:
  ```bash
  gpg --full-generate-key                        # RSA 4096, with a passphrase
  gpg --list-secret-keys --keyid-format=long
  gpg --keyserver keyserver.ubuntu.com --send-keys <KEYID>
  gpg --armor --export-secret-keys <KEYID>       # the value of MAVEN_GPG_PRIVATE_KEY
  ```
- [ ] GitHub: *Settings → Environments → `maven-release`*, with required reviewers and these secrets:
  - [ ] `MAVEN_CENTRAL_USERNAME`
  - [ ] `MAVEN_CENTRAL_PASSWORD`
  - [ ] `MAVEN_GPG_PRIVATE_KEY`
  - [ ] `MAVEN_GPG_PASSPHRASE`

## 4. First release (a release candidate; Central releases can never be replaced)

- [ ] Set `version = "0.3.0-rc.1"` in `Cargo.toml`, run `cargo update --workspace`, update `CHANGELOG.md`, and merge.
- [ ] Dry run: *Actions → Maven release review → Run workflow* on `main`, with **publish unchecked**. Every job must pass, including the staged-release tests on all seven platforms.
- [ ] `git tag v0.3.0-rc.1 && git push origin v0.3.0-rc.1`
- [ ] Run *Maven release review* from the tag with **publish checked**, then approve the `maven-release` environment.
- [ ] Check the deployment on central.sonatype.com (*Deployments*). Search can take from 30 minutes to a few hours to show it.
- [ ] Verify it as a user would: add the plugin to a fresh project and run `mvn verify` (README → Maven).
- [ ] Repeat with the final version, `0.3.0`.

## 5. Other channels (only when releasing them at the same version)

- [ ] npm: bootstrap trusted publishing for the new packages `dowsing-rod-native-linux-x64-musl` and `dowsing-rod-native-linux-arm64-musl` (temporary `NPM_TOKEN`, then configure the publisher). `npm-release.yml` now expects 8 tarballs.
- [ ] NuGet: nothing new. The tool package now also carries `linux-musl-x64` and `linux-musl-arm64`.
- [ ] PyPI: nothing new. Java support comes with the wheel; there are no musllinux wheels yet.

## 6. Follow-ups (optional)

- [ ] Gradle plugin on the Gradle Plugin Portal, reusing `dowsing-rod-java`
- [ ] `dowsing-rod-cli` executable jar and a JBang catalog (`jbang dowsing-rod@… scan .`)
- [ ] SARIF output for GitHub code scanning
- [ ] musllinux Python wheels
- [ ] Windows arm64 binaries
- [ ] Kotlin frontend (`tree-sitter-kotlin`)
- [ ] Configurable cache directory (for example under `target/`)
- [ ] Tune Java classification on real repositories (Commons Lang, Spring PetClinic, a large multi-module project) and record precision in the action plan
