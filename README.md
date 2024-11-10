STM data generator for probe-rs
===============================

This tool uses [stm32-data-sources](https://github.com/embassy-rs/stm32-data-sources) and some
custom code to generate presumably complete device lists and correct memory maps for [probe-rs](https://github.com/probe-rs/probe-rs).

To use the tool, update commit hashes in `d.ps1`, then run `./d.ps1 download-all` and `./d.ps1 gen`.

Then, use the contents of the `output` folder to update probe-rs.

Please review changes carefully. Any change in the memory map needs to be double-checked.
