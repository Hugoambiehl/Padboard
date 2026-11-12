# Contributing to Padboard

Thank you for your interest! Bug reports, ideas, translations and code are
all welcome.

## Reporting a bug or asking for a feature

Open an [issue](https://github.com/Hugoambiehl/Padboard/issues/new/choose)
with the matching template. For a bug, give the versions (desktop app:
*Connections* → *About*; tablet: *About and licenses*), the operating
system, the tablet model, what you did and what happened. The desktop log
helps a lot: `padboard.log` in `%APPDATA%\io.github.hugoambiehl.padboard\`
(Windows) or `~/.config/io.github.hugoambiehl.padboard/` (Linux). Check it
for anything private before attaching it.

**Security issues**: never in a public issue, see [SECURITY.md](SECURITY.md).

## Before writing code

For anything bigger than a small fix, open an issue first to agree on the
approach: it avoids work that cannot be merged. A few principles shape the
project:

- **Free software only.** Every tool, library and API must be free and open
  source. No paid service, nothing that needs an account or an approval, no
  Google Play Services, Firebase or ML Kit on the tablet. Dependencies must
  be under a GPL-compatible license; the allow-lists are enforced by the
  build ([development guide](docs/development.md#third-party-licenses)).
- **The desktop is the brain, the tablet is a screen.** All configuration
  and logic live in the desktop app; the tablet draws what it receives and
  sends presses back.
- **The protocol lives in `crates/protocol`**, the single source of truth.
  Change it there, then regenerate the TypeScript and Kotlin types with
  `scripts/gen-types.sh` (never edit generated files). Breaking changes bump
  the protocol version, with round-trip serialization tests.
- **The tablet must stay smooth** on modest hardware: animations only
  change `transform` and `alpha` (`graphicsLayer`), never the layout or a
  blur, and nothing animates forever.
- **No hard-coded UI text**: every string goes in both languages
  (`apps/desktop/src/locales/{en,fr}.json`,
  `apps/android/app/src/main/res/values{,-fr}/strings.xml`). A test checks
  that both desktop files have the same keys.
- **Actions** have real implementations on Windows and Linux. An action
  that cannot work on a platform logs it and returns an error; it never
  panics.

## Making a change

1. Fork the repository and create a branch from `main`.
2. Set up your environment with the [development guide](docs/development.md).
3. Make the change, with tests: protocol round trips, configuration logic,
   frontend logic (Vitest), the connection manager on Android. For UI
   changes, run the end-to-end tests (`scripts/desktop-e2e.sh`) and look at
   their screenshots.
4. Run the checks of the [development guide](docs/development.md#checks):
   formatting (`cargo fmt`, Prettier, ktlint), lints (`cargo clippy -D
   warnings`, ESLint, Android Lint) and tests. The CI runs them on your pull
   request.
5. Open a pull request that explains what changes and why, and how you
   tested it (which platforms, which tablet).

### Commits

Small commits, in English, following
[Conventional Commits](https://www.conventionalcommits.org):

```
feat(tablet): show the battery level in the top bar
fix(actions): release held keys when a shortcut fails
```

Types: `feat`, `fix`, `perf`, `refactor`, `docs`, `test`, `build`, `ci`,
`chore`, `style`. The changelog is generated from them: `feat`, `fix` and
`perf` commits are listed, so write their summary for users.

### Code style

- Rust: `cargo fmt`, `cargo clippy -- -D warnings` (the workspace lints).
- TypeScript and Vue: ESLint and Prettier (`npm run lint`,
  `npm run format`).
- Kotlin: ktlint (`./gradlew ktlintFormat`). Dependencies are declared in
  `gradle/libs.versions.toml` only, and must support Android 8 (minSdk 26).
- Paths in the repository may contain spaces on contributors' machines:
  quote them in scripts.

## License of contributions

Padboard is licensed under the GPL-3.0-or-later. By submitting a
contribution, you agree that it is released under the same license.
