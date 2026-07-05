# Security Policy

## Supported versions

| Version | Supported |
|---|---|
| 0.1.x (latest release) | ✅ |
| older | ❌ — upgrade to the latest release |

While Snowbros Atlas is on 0.x, only the most recent release receives
security fixes.

## Reporting a vulnerability

Please **do not open a public issue** for security problems.

Report privately via **GitHub Security Advisories**:
[Security → Report a vulnerability](https://github.com/snowbros/atlas/security/advisories/new)
on the repository. If you cannot use GitHub, email
**security@snowbros.me**.

Include: affected version, platform, reproduction steps or proof of
concept, and impact assessment if you have one.

In scope, for example:

- Analysis of a malicious repository escaping the analyzer (arbitrary
  code execution, path traversal via crafted imports/tsconfig).
- `sb fix` writing outside the project root or clobbering unintended
  files.
- Secret values leaking unredacted into any output format or the cache.
- npm wrapper: checksum bypass or binary substitution during download.

## Response expectations

- Acknowledgement within **72 hours**.
- Initial assessment within **7 days**.
- Fix or mitigation plan for confirmed vulnerabilities within **30
  days**; critical issues are prioritized ahead of all other work.

## Disclosure policy

Coordinated disclosure: we ask that you give us the response windows
above before publishing details. We will credit reporters in the release
notes and advisory unless you prefer otherwise. Once a fix ships, we
publish a GitHub Security Advisory with affected versions and upgrade
guidance.
