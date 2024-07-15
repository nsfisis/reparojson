# ReparoJSON

A simple command-line tool to "repair" JSON. It only fixes the syntactic errors and never formats the given input.



## Usage

```
Usage: reparojson [OPTIONS] [FILE]

Arguments:
  [FILE]  The input JSON file (default: STDIN)

Options:
  -q, --quiet    Successfully exit if the input JSON is repaired
  -h, --help     Print help
  -V, --version  Print version
```


## Examples

```
$ echo '[ 1 2 ]' | reparojson
[ 1, 2 ]

$ echo '[ 1, 2, ]' | reparojson
[ 1, 2 ]

$ echo '{ "foo": 1 "bar": 2 }' | reparojson
{ "foo": 1 ,"bar": 2 }

$ echo '{ "foo": 1, "bar": 2, }' | reparojson
{ "foo": 1, "bar": 2 }
```


## License

See (LICENSE)[./LICENSE].
