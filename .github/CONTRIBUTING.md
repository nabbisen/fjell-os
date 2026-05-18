# Contributing

Thanks for your interest in fjell OS.

We are a small, independent open-source community building a memory-safe, capability-based microkernel OS. We welcome bug reports, questions, discussions, and code contributions.

Please understand that this is a small project. Maintainers have limited time and cannot guarantee immediate responses or merge every feature request. We prioritize architectural integrity, stability, and small security boundaries over rapid feature expansion.

## 1. Reporting Bugs and Weird Behavior

When an operating system does something unexpected, it can be hard to track down. We appreciate detailed reports.

If you hit a kernel panic, a capability denial that shouldn't happen, or a boot failure:
- **Search existing issues** to see if it's already reported.
- **Open a Bug Report** using the issue tracker.
- **Include logs:** Serial console output, QEMU panic traces, or audit logs (`auditd` output) are vital.
- **Provide reproduction steps:** A minimal QEMU command or a specific TOML configuration that triggers the issue is the fastest way to get it fixed. Mention your host OS and QEMU/toolchain versions.

## 2. Asking Questions (Q&A)

Operating system development—especially involving formal capabilities and Rust `no_std` environments—has a steep learning curve. 

If you are stuck, confused by the architecture, or just want to ask "How does Fjell OS handle X?":
- **Open a Discussion or Issue** tagged as `question`.
- We are happy to explain the design. Sometimes a good question points out a gap in our documentation that we need to fix.
- Do not feel like you need a fully formed PR to start a conversation.

## 3. Proposing Code Changes (Before you start)

For anything beyond fixing a typo or a minor documentation update, **please open an issue first** and describe what you want to change and why.

This is not a barrier to entry. It is a safeguard. Operating systems have complex internal invariants (especially around IPC, capability revocation, and memory mapping). By discussing your idea first, we can say "yes, please send a PR" and agree on the approach, saving you from writing code that we cannot safely merge.

## 4. Code Style & Tooling

We enforce strict formatting and linting to keep the codebase readable and safe.

- Run `cargo fmt` before pushing.
- `cargo clippy --workspace --all-targets` must be clean.
- `cargo test --workspace` must pass.
- **`unsafe` policy:** The `unsafe` keyword is heavily restricted. It is completely forbidden in user-space services. In `fjell-kernel` or `fjell-arch`, any new `unsafe` block **must** include a `// SAFETY:` comment explicitly detailing the invariants the caller or the hardware guarantees.
- **Lints:** We use workspace-wide lints. `unwrap_used` and `expect_used` are warnings. If you must use `expect`, you must include a comment explaining exactly why it is mathematically or structurally impossible to fail.
- **Documentation:** Public items in library crates require rustdoc.

## 5. Testing (Unit, Integration, and Property)

An OS must be reliable. We require tests for new behavior.

- **Unit tests:** Live next to the code (`mod tests`).
- **Integration tests / Negative tests:** Live in the `tests/` directory or run via our QEMU test harness (`cargo xtask qemu-negative`). 
- **Property-based tests:** We use [proptest](https://crates.io/crates/proptest) to verify logical invariants (e.g., capability derivation rules, IPC state machines). If a property fails and generates a regression file under `proptest-regressions/`, **commit that file**.

When adding a bug fix, please add a test (often a QEMU smoke test or a negative test) that fails on `main` and passes on your branch.

## 6. Commit Hygiene

- One logical concern per commit.
- Squash WIP (Work In Progress) commits before opening a PR.
- Use the imperative mood in the subject line (e.g., "Add RISC-V PMP initialization" not "Added...").
- Reference the issue number in the body if relevant.

## 7. What We Look For in a PR

- A clear description of the problem being solved.
- A test that exercises the new behavior (especially negative tests for security boundaries).
- A note in `CHANGELOG.md` under the `[Unreleased]` section if the change affects users, operators, or the system ABI.
- Updates to the `docs/` directory if you are changing architectural boundaries, the Intent Stream schema, or operational commands.

## 8. What We Won't Merge

Because Fjell OS aims for minimal complexity and verifiable security, we will reject:
- Code that bypasses capability checks for "convenience."
- Expansions of the kernel's responsibilities (policy belongs in user-space).
- Large new subsystems (like full network stacks or POSIX filesystems) that have not been agreed upon in an architecture issue.
- Unjustified external dependencies, particularly those that pull in transitive `unsafe` code.
- Features that break the append-only state model or the immutable upgrade model without a heavily discussed migration plan.

## 9. Code of Conduct

Be kind, patient, and assume good intent. Argue with the idea, not the person. Operating systems are hard, and debugging hardware traps is frustrating. Let's keep the community supportive and focused on building a great system together.
