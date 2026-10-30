<div align="center">
  <a href="README.md"><b>DLGE Player</b></a>
</div>

# Contributing to DLGE Player

Thanks for your interest in DLGE Player, an open-source player for
Diabolik Lovers Grand Edition.
Contributions of all kinds are welcome: format notes, opcode meanings, bug
reports, documentation fixes, and arguments against a decision that is
already made.

The toolchain is Rust 1.88 or later.

    cargo fmt --all --check
    cargo clippy --workspace --all-targets -- -D warnings
    cargo test --workspace

Those three are the gate. They run on Windows, macOS, and Linux.
The [README](README.md) says what is built so far.

## Ways to contribute

- Find the meaning of an opcode. Give the opcode, the scripts where you see
  it, and the behaviour in the game that proves the meaning.
- Describe a part of the Bezel Engine `.bea` format.
- Report a script, an archive, or a dump that the tools read badly.
- Improve the documentation.
- Take on a phase from [docs/roadmap.md](docs/roadmap.md), or argue that the
  order is wrong, with the reason.

Before you start significant work, open an issue to agree on the approach.
This costs you one message and can save you a rewritten pull request.

## The rule that comes before all the others

**The repository never holds game data.**

This includes the scripts, the images, the audio, the movies, the fonts, the
text of the game, the files of a patch, and the keys of a console. It also
includes a small part of a file, a hex dump of a file, and a test fixture
that you cut out of a file.

A pull request that adds such data is refused. This holds even when the data
is small, and even when the test needs it.

## How a test gets its data

A test builds the bytes that it needs inside itself.
The `cpk` tests make their own archives with the `@UTF` writer and the
CRILAYLA encoder. The `stcm2l` tests make their own scripts with the writer,
and one test writes the bytes by hand so that it does not depend on the
writer.

The corpus test is the one exception. It reads the real scripts of a dump
only when `DLGE_SCRIPTS` points to them, and it does nothing without the
variable. It runs on your computer, never in CI.

    DLGE_SCRIPTS=path/to/scripts cargo test -p stcm2l --test corpus

## Three traps that this project creates

**A round trip can pass for the wrong reason.**
A parser and a writer that share one wrong idea agree with each other.
Add a test that reads bytes made by hand, and run the corpus test on a real
dump before you say that a format change is correct.

**A guess is not a meaning.**
An opcode that shows text in one script can do something else in another.
Give the evidence for each meaning in `docs/formats.md`, and mark a meaning
that is not verified.

**A count of some files is not a count of all files.**
Say how many scripts a change reads correctly, out of how many.
Give the number that the tool prints, not a number that you calculated from
a sample.

## Coding style

- Match the surrounding code. Small, focused functions and clear names beat
  cleverness.
- A parser never panics on bad input. It returns an error.
- Add dependencies sparingly, and say in the pull request why the standard
  library or an existing dependency does not do the job.
- The project is GPL-3.0-or-later. A dependency must have a licence that is
  compatible with it.
- Do not copy code from a project that has no licence. Use its notes, and
  write the code again.

## Commit messages and pull requests

[`AGENTS.md`](AGENTS.md) is the full set of rules. These four cause the most
rework.

- **One change per commit, and a feature is many commits.** A commit that says
  "integrate the full feature" is wrong even when the code is right. Split it
  into the steps that a reviewer can read and revert one at a time.
- **Code and its tests go in one commit. Documentation goes in its own.**
- **All text uses Simplified Technical English.** Short sentences, active voice,
  present tense. No em-dashes and no emoji, in source, comments, documentation,
  commit messages, or pull requests.
- **Write the subject in the present tense, with no version number.** A commit
  changes no version.

In the pull request, describe what changed and why, and say how you tested it.
If you ran the corpus test, say so, and give the count that it printed.

## Reporting security issues

Do not open a public issue for a security problem.
See [SECURITY.md](SECURITY.md) for how to report it privately.

## Code of conduct

By taking part in this project you agree to follow the
[Code of Conduct](CODE_OF_CONDUCT.md).
