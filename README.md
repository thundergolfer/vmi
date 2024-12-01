# `vmi`

`vmi` is a CLI tool for working with Virtual Machine Images (VMIs).
It has support for interacting with cloud provider VMI objects, and the
most important open VMI formats.

**Supported Cloud VMI Formats:** AWS EC2 AMI, GCP GCE Images.

**Supported Open VMI Formats:** Raw, VMDK, and OVF.

## Usage


```
Virtual machine images made simple!

Usage: vmi [OPTIONS] <COMMAND>

Commands:
  convert  Convert between virtual machine image formats
  inspect  Return information on virtual machine images
  help     Print this message or the help of the given subcommand(s)

Options:
  -v, --verbose <VERBOSE>  Verbosity level (can be specified multiple times) [default: 1]
  -h, --help               Print help
  -V, --version            Print version
```
