# Real-world results

What Dowsing Rod reports on well-known open-source projects, how long it takes, and where it is still weak. Every number here can be reproduced with the commands at the end.

## What was scanned

| Project | Language | Commit | Files | Functions |
|---|---|---|---|---|
| [spring-projects/spring-petclinic](https://github.com/spring-projects/spring-petclinic) | Java | `3aae6cc` (2026-11-26) | 50 | 187 |
| [apache/commons-lang](https://github.com/apache/commons-lang) | Java | `657ebb3` (2026-11-26) | 648 | 14,220 |
| [google/guava](https://github.com/google/guava), without `android/` | Java | `cdb3863` (2026-11-25) | 1,678 | 31,892 |
| [pallets/flask](https://github.com/pallets/flask) | Python | `49943b5` (2026-10-30) | 83 | 1,460 |

Scanner: 0.3.0-rc.3, default settings, cache off. The Java projects were scanned through the Maven plugin from Maven Central, Flask through the CLI; both run the same native binary. Machine: a Windows 11 laptop with an Intel Core i5-1135G7 (4 cores, 8 threads).

Guava's `android/` tree is a near-copy of `guava/`. With it included, Dowsing Rod correctly pairs the two copies (9,578 clusters), which is accurate but not useful, so it is excluded as a user would exclude it.

## Summary

| Project | Scan time | Candidate pairs scored | Clusters | High value | Files with parse errors |
|---|---|---|---|---|---|
| spring-petclinic | 0.2 s | 15,395 | 22 | 0 | 0 |
| commons-lang | 2.3 s | 53,731 | 1,750 | 138 | 1 (an OSS-Fuzz test of deliberately invalid input) |
| guava | 5.5 s | 136,287 | 4,578 | 179 | 0 |
| flask | 0.2 s | 2,092 | 147 | 1 | 0 |

Scan time is the scanner's own time without the cache. A Maven run adds a few seconds of JVM and plugin start-up, and a second scan of unchanged files is faster because of the cache.

## Does it find the duplication that is known to be there?

Both Java libraries contain duplication that their authors keep on purpose: Java generics do not cover primitive types, so the same method is written once per type (`int[]`, `long[]`, `char[]`, …). This makes a good ground truth: every one of these overloads should be clustered with its siblings.

| Known duplicate family | Clustered |
|---|---|
| commons-lang `ArrayUtils`: `reverse`, `subarray`, `removeElement`, `contains` | 18/18, 9/9, 9/9, 10/10 |
| commons-lang `ArrayUtils`: `indexOf`, `toPrimitive`, `add` | 15/20, 14/17, 13/19 |
| Guava `primitives` (`Ints`, `Longs`, `Shorts`, `Chars`, `Bytes`, `Doubles`, `Floats`): `indexOf`, `contains`, `concat`, `ensureCapacity`, `reverse`, `rotate`, `toArray`, `max`, `min` | all (35/35, 14/14, 7/7, 7/7, 14/14, 14/14, 7/7, 6/6, 6/6) |
| Guava `primitives`: `lastIndexOf`, `join` | 20/21, 5/6 |

Most overloads that are not clustered are 3–5 line delegations such as `return indexOf(array, valueToFind, 0);`, which fall under the minimum function size, or variants with extra parameters (an insertion index, a tolerance, a component class) that change their structure.

## Examples of findings

### Guava: one algorithm, eight copies

Cluster of 8 `rotate` methods in `Booleans`, `Bytes`, `Chars`, `Doubles`, `Floats`, `Ints`, `Longs`, and `Shorts`, similarity 0.99, classified *near duplicate*. Two of them:

```java
// guava/src/com/google/common/primitives/Bytes.java
public static void rotate(byte[] array, int distance, int fromIndex, int toIndex) {
  checkNotNull(array);
  checkPositionIndexes(fromIndex, toIndex, array.length);
  int length = toIndex - fromIndex;
  if (length <= 1) {
    return;
  }
  distance = floorMod(distance, length);
  ...

// guava/src/com/google/common/primitives/Chars.java
public static void rotate(char[] array, int distance, int fromIndex, int toIndex) {
  checkNotNull(array);
  checkPositionIndexes(fromIndex, toIndex, array.length);
  ...
```

Other top findings in Guava's production code: `Multisets.union`, `intersection`, `sum`, and `difference` (8 methods, 0.94); `indexOf` and `lastIndexOf` across the primitive classes (17 methods, 0.98); and the `readObject`/`writeReplace` serialization boilerplate of the immutable collections.

### Commons Lang: the largest clusters are the real ones

The highest-value clusters in `src/main` are all genuine families of copies:

| Cluster | Members | Similarity | Classification |
|---|---|---|---|
| `StringUtils.join` overloads | 16 | 0.99 | near duplicate |
| `Conversion.binaryToByte`, `binaryToInt`, `binaryToLong`, `binaryToShort`, … | 20 | 0.94 | factory candidate |
| `ArrayUtils.shift` | 9 | 1.00 | near duplicate |
| `ArrayUtils.insert` | 9 | 0.97 | duplicate |
| `CompareToBuilder.append` | 9 | 0.98 | near duplicate |
| `ArrayUtils.indexesOf` | 10 | 0.98 | near duplicate |

### Spring PetClinic: copy-paste between controllers

`OwnerController.addPaginationModel` and `VetController.addPaginationModel` are classified *duplicate* (0.96). Only the entity type and two strings differ:

```java
// OwnerController.java
private String addPaginationModel(int page, Model model, Page<Owner> paginated) {
    List<Owner> listOwners = paginated.getContent();
    model.addAttribute("currentPage", page);
    model.addAttribute("totalPages", paginated.getTotalPages());
    model.addAttribute("totalItems", paginated.getTotalElements());
    model.addAttribute("listOwners", listOwners);
    return "owners/ownersList";
}

// VetController.java
private String addPaginationModel(int page, Page<Vet> paginated, Model model) {
    List<Vet> listVets = paginated.getContent();
    model.addAttribute("currentPage", page);
    model.addAttribute("totalPages", paginated.getTotalPages());
    model.addAttribute("totalItems", paginated.getTotalElements());
    model.addAttribute("listVets", listVets);
    return "vets/vetList";
}
```

Note that the two methods do not even take their parameters in the same order; Dowsing Rod compares structure, not text. Other PetClinic findings: the same `orElseThrow(() -> new IllegalArgumentException(...))` lambda in five places, `processCreationForm` vs `processUpdateForm` in `PetController`, and identical `setAllowedFields` binders in three controllers.

### Flask: three decorators with one body

`App.template_filter`, `App.template_test`, and `App.template_global` (`src/flask/sansio/app.py`) form a *strategy candidate* cluster: each checks whether it was called without parentheses, registers the function, and returns a decorator, differing only in the `add_template_*` method they call.

## Known limitations

These are visible in the reports above and are worth knowing before you act on one:

- **Test code dominates by volume.** Test methods are often written from a template, so most clusters in a large project are in tests (1,161 of Commons Lang's 1,750). They are real, but duplication in tests is often intentional. Exclude test folders (`exclude = ["**/src/test/**"]`) when you only want production code.
- **Small Python functions still produce noise.** Java, C#, TypeScript, JavaScript, C, C++, and Rust skip functions under 30 normalized tokens. Python does not yet, because its normalizer produces fewer tokens per line and a single threshold removed real 6-line duplicates. In Flask this shows up as clusters of one-line methods such as `SessionInterface.get_cookie_name` and `get_cookie_domain`.
- **Labels are coarse.** Most clusters are labelled *near duplicate*. The classification is a hint about the shape of the refactoring; the members and the listed differences are what to review.
- **Large families are split.** A cluster holds at most 20 functions, so a family of 40 copies appears as two or more clusters.
- **Some idioms cluster across unrelated classes**, for example `equals` implementations, because they share a structure. They rank lower than real copies but appear in large reports.

## How this was found and fixed

These results come from 0.3.0-rc.3. The same test of 0.3.0-rc.2 exposed a defect: candidate generation stopped at 25,000 pairs per language and filled that budget with small test methods, so the families above were almost entirely missed.

| | 0.3.0-rc.2 | 0.3.0-rc.3 |
|---|---|---|
| commons-lang `ArrayUtils` `reverse` / `subarray` / `contains` | 0/18, 0/9, 3/10 | 18/18, 9/9, 10/10 |
| Guava `primitives` families clustered | 5 of 137 | 135 of 137 |
| Guava files with parse errors | 7 | 0 |
| Guava high-value clusters | 28 | 179 |

rc.3 gives every function its own budget of nearest neighbours, parses annotated varargs (`Object @Nullable ... args`), and skips trivial functions; see [CHANGELOG.md](../CHANGELOG.md) and [algorithms.md](algorithms.md#candidate-generation).

## Reproduce

Clone the projects at the commits above. A shallow clone is enough, and nothing in the clone is modified except the scan cache, which writes its own `.gitignore`.

```bash
git clone --depth 1 https://github.com/apache/commons-lang.git
cd commons-lang
```

**Maven** (Java projects; Java 8+ and Maven 3.6.3+). On Windows PowerShell, quote each `-D` argument.

```bash
mvn io.github.shkb42:dowsing-rod-maven-plugin:0.3.0:scan "-Ddowsing-rod.useCache=false"
mvn io.github.shkb42:dowsing-rod-maven-plugin:0.3.0:scan "-Ddowsing-rod.format=json" "-Ddowsing-rod.outputFile=target/dowsing-rod.json"
# Guava: skip the android/ copy
mvn io.github.shkb42:dowsing-rod-maven-plugin:0.3.0:scan "-Ddowsing-rod.excludes=android/**"
```

Some projects need a newer JDK just to load their POM; Spring PetClinic needs Java 17+ because of a build extension. The scanner itself does not; the plugin also runs without any `pom.xml`, from any folder: `mvn io.github.shkb42:dowsing-rod-maven-plugin:0.3.0:scan "-Ddowsing-rod.path=/path/to/project"`.

**CLI** (any language; install it from PyPI, npm, or NuGet as described in the [README](../README.md#install), version 0.3.0 or later for Java):

```bash
dowsing-rod scan . --no-cache
dowsing-rod scan . --format json --no-cache > dowsing-rod.json
dowsing-rod scan . --exclude 'android/**' --no-cache     # Guava
```

**Python API:**

```python
import dowsing_rod

result = dowsing_rod.scan(".", no_cache=True)
for cluster in result["clusters"][:10]:
    print(cluster["id"], cluster["classification"], cluster["refactoring_value"])
```

To check the numbers in a JSON report, `statistics` has the file, function, pair, and cluster counts, and each cluster's `function_indices` point into `functions`, which give the file and line range of every member.

---

*Code excerpts are from Guava and Spring PetClinic (Apache License 2.0) and are shown only to illustrate the findings.*
