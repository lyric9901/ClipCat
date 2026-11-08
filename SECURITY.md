# Security Policy

## Supported Versions

ClipCat is under active development. Fixes go into the current minor release only; older releases are not patched.

| Version | Supported          |
| ------- | ------------------ |
| 0.2.x   | :white_check_mark: |
| < 0.2   | :x:                |

## Reporting a Vulnerability

Please do not open a public issue for a security vulnerability.

Report it privately through GitHub's advisory form:
https://github.com/lyric9901/ClipCat/security/advisories/new

Include the version (Settings › About), the platform, and steps to
reproduce. A proof of concept helps; a project file that triggers the
problem helps most.

What to expect:

- An acknowledgement within 3 business days.
- An update at least once a week while the report is open.
- If the report is accepted: a fix in the next release, a published
  advisory, and credit to you unless you would rather not be named.
- If it is declined: a transparent explanation of why.

## Threat Model & Invariants

ClipCat runs entirely on your local machine with an offline-first philosophy. Security issues of primary focus:

- A project file, media file, `.concatpack` template or effect package that causes
  code execution, writes outside the project folder (path traversal), or reads files it
  was not given.
- An asset pack or downloaded model being swapped without the digest check catching it.
- The API server (`concat-cli serve`, or Settings › Automation) accepting a
  call it should have refused: a missing or wrong token, on any address,
  or a server that started with no token at all.


By design, and not a vulnerability: the API server does whatever the
person who started it can do on that machine, and it is not encrypted.
Every connection presents a token, minted when none was set; binding to
anything other than 127.0.0.1 should sit behind something that provides
transport security.
