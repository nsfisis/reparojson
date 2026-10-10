# Changelog

## Unreleased

* Added `--generate-completion <SHELL>` option to generate the shell completion script.
* Fixed excessive repair: a missing comma is no longer inserted between numbers or literals (`null`, `true`, `false`) that have no whitespace between them. For example, `[1+2]` was repaired into `[1,2]`, but is now rejected as invalid.
* Fixed a number with multiple minus signs (e.g., `[--1]`) being accepted as valid and output as is. It is now rejected as invalid.

## v1.0.0

First stable version released!

Notable changes since v0.x:

* **BREAKING**: ReparoJSON now exits with 0 when the input is successfully repaired. `-q`/`--quiet` is removed as it is the default behavior now.
* **BREAKING**: Exit codes on failure are no longer distinguished (previously 2 for invalid input and 3 for I/O errors). Only zero vs. non-zero is specified.
* **BREAKING**: The short flag to print version is changed from `-V` to `-v`.
* Added `-s`/`--strict` flag to exit with failure if the input is repaired.
* Added `-i`/`--in-place` flag to replace the input file in place.
