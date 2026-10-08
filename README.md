# ReparoJSON

A simple command-line tool to "repair" JSON. It only fixes the syntactic errors and never formats the given input.



## Usage

```
Usage: reparojson [OPTIONS] [FILE]

Arguments:
  [FILE]  The input JSON file (default: STDIN)

Options:
  -q, --quiet    Successfully exit if the input JSON is repaired
  -v, --version  Print version
  -h, --help     Print help
```


## Examples

```
$ echo '[ 1 2 ]' | reparojson
[ 1, 2 ]

$ echo '[ 1, 2, ]' | reparojson
[ 1, 2 ]

$ echo '{ "foo": 1 "bar": 2 }' | reparojson
{ "foo": 1, "bar": 2 }

$ echo '{ "foo": 1, "bar" 2, }' | reparojson
{ "foo": 1, "bar": 2 }
```


## Editor Integration Examples

### Neovim v0.11+ with efm-langserver

```lua
vim.lsp.config('efm', {
   cmd = { 'efm-langserver' },
   filetypes = { 'json' },
   init_options = { documentFormatting = true },
   settings = {
      rootMarkers = { ".git/" },
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
vim.lsp.enable('efm')
```


## License

See [LICENSE](./LICENSE).
