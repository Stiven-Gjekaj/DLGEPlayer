<div align="center">
  <a href="README.md"><b>DLGE Player</b></a>
</div>

# Security Policy

## Supported versions

DLGE Player has no release yet.
When it has one, fixes go to the latest version on the default branch.
Older versions are not maintained.

## Reporting a vulnerability

Report security problems privately, and not through a public issue.

- Preferred: open a private security advisory with the "Report a vulnerability"
  button on the Security tab of this repository.
- Alternative: email the maintainer at stivenagostingjekaj@gmail.com.

Include the steps to reproduce, the affected commit, and the impact as you
understand it.
You can expect a first answer within a few days.
Your report gets an acknowledgement when the fix ships, unless you prefer to
stay anonymous.

## The threat model

**The input is untrusted.**
The tools read archives, compressed data, and scripts.
A user can get a dump or a patch from somebody else.
A crafted file can try to crash a parser, make it read or write outside its
buffer, or make it use all the memory.

**The importer writes files.**
A path inside an archive can try to write outside the output directory.

**These are in scope:**

- A file that makes a parser panic, hang, or read or write outside its buffer.
- A file that makes a tool allocate memory far beyond the size of the file.
- A path in an archive that writes outside the output directory.
- A script that makes the VM run code or read files outside the imported
  folder.

**These are out of scope:**

- A report that the tools read game data. That is what they do.
- A file that the tools refuse with an error. That is the correct result.
- Findings from an automated scanner with no working demonstration.
