# Security policy

fjell OS handles authentication, so security reports get a real response.

## Reporting a vulnerability

If you believe you have found a security issue in fjell OS:

1. **Do not file a public GitHub issue.** Public issues become indexable
   immediately and put other operators at risk.
2. **Open a [private security advisory](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability)**
   on the repository: https://github.com/nabbisen/fjell-os/security/advisories/new
3. Include enough information for the maintainers to reproduce: version,
   configuration, and a minimal example.

## What you can expect

- An acknowledgement within a small number of days.
- A discussion of severity and timeline before any public disclosure.
- Credit in the changelog and security advisory unless you ask for
  anonymity.

## What's in scope

Anything that allows:

- Memory leakage or broken.
- Authentication bypass.
- Privilege escalation.
- Data breach or broken.
- Denial of service.

## Out of scope

- Findings that require already having root on the host.
- Findings that depend on the operator misusing a configuration option that
  is documented as dangerous.
- Best-practice nits on otherwise-safe code (please file a normal issue or
  PR).
- Vulnerabilities in upstream Rust crates — those should go to the relevant
  upstream project, though we appreciate a heads-up.
