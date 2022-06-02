# gone

Delete all untagged versions of GitHub container packages.

## Usage

```
gone 0.1.0
Delete all untagged versions of GitHub container packages

USAGE:
    gone [OPTIONS] <PACKAGE_NAMES>...

ARGS:
    <PACKAGE_NAMES>...    Packages to clean

OPTIONS:
    -h, --help             Print help information
    -n, --dry-run          
        --org <ORG>        Organization owning the packages (conflicts with --user)
        --token <TOKEN>    Path to a file containing a GitHub token. You can also pass a token
                           verbatim via the GITHUB_TOKEN env variable
        --user <USER>      User owning the packages (conflicts with --org)
    -v, --verbose          
    -V, --version          Print version information
```
