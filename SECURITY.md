# Security

Report a vulnerability privately with **Report a vulnerability** on the repository's [Security tab](https://github.com/mattbaconz/jarsnitch/security). That covers a jar that crashes or hangs the scanner, gets past its size limits, or runs anything in the browser page beyond the analysis.

jarsnitch reads the jars it scans and never loads or runs them. The command line tool doesn't use the network. The web page fetches its own files once and then nothing else, and your jars never leave your machine.

A plugin jarsnitch doesn't flag isn't a security bug in jarsnitch: [the README lists what it can't catch](README.md#what-it-cant-catch). To suggest a rule, open an issue. Don't attach or link real malware; describe the behavior or link a public write-up.

Fixes ship in the latest release only.
