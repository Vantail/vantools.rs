# Security Policy

## Supported Versions

Security fixes are applied to the latest commit on the default branch. No
backporting is provided.

| Version                          | Receives security updates |
| -------------------------------- | ------------------------- |
| Latest commit on `main`          | ✅                        |
| Previous commits, forks, and unversioned builds | ❌         |

## Reporting a Vulnerability

**Do not open a public issue for a security vulnerability.**

If GitHub's private vulnerability reporting is enabled for this repository, use
it: open the **Security** tab and choose **Report a vulnerability**. The report
stays private between you and the maintainers.

If private vulnerability reporting is not available, contact the maintainers
directly and privately through their GitHub accounts:

- Jeroen van Wissen (@jeroenvanwissen)
- f0xb17 (@f0xb17)

Use a private message rather than a public issue, pull request, or discussion.
Do not include exploit details in public channels.

## What to Include

Please provide:

- A description of the vulnerability and the affected component or file.
- Step-by-step reproduction instructions, including the input that triggers it.
- The potential impact, such as data loss, code execution, or denial of
  service, and the conditions required to exploit it.
- A proof of concept when you have one.
- Any suggested remediation, if you have considered it.

## What to Expect

The maintainers aim to acknowledge reports within **7 days**. Please allow
**90 days** for a fix before public disclosure.

These are target timelines, not guarantees. Remediation time may depend on the
severity of the issue, the complexity of the fix, and the availability of the
maintainers. We will keep you informed of progress and will coordinate
disclosure timing with you.

## Out of Scope

The following are out of scope for this policy:

- **Issues in third-party dependencies.** Report these to the upstream project
  that maintains the dependency. If a dependency issue can be used to
  compromise this project, tell us anyway so we can assess our exposure and
  update, pin, or mitigate.
- **Social engineering** of maintainers or contributors.
- **Physical-access attacks.**

Vulnerabilities in third-party dependencies are still worth reporting and
addressing; they are simply handled through the upstream project rather than
through this policy.
