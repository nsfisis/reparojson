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


## Editor Integration Examples

### Neovim + nvim-lspconfig + efm-langserver

```lua
local lspconfig = require('lspconfig')
lspconfig.efm.setup({
   init_options = { documentFormatting = true },
   settings = {
      rootMarkers = {".git/"},
      languages = {
         json = {
            {
               formatCommand = "reparojson -q",
               formatStdin = true,
            },
         },
      },
   }
})
```


## License

See [LICENSE](./LICENSE).
