# Changelog

## Unreleased

* Added `--generate-completion <SHELL>` option to generate the shell completion script.

## v1.0.0

First stable version released!

Notable changes since v0.x:

* **BREAKING**: ReparoJSON now exits with 0 when the input is successfully repaired. `-q`/`--quiet` is removed as it is the default behavior now.
* **BREAKING**: Exit codes on failure are no longer distinguished (previously 2 for invalid input and 3 for I/O errors). Only zero vs. non-zero is specified.
* **BREAKING**: The short flag to print version is changed from `-V` to `-v`.
* Added `-s`/`--strict` flag to exit with failure if the input is repaired.
* Added `-i`/`--in-place` flag to replace the input file in place.
