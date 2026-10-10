# What Can Be Repaired

ReparoJSON repairs the following mistakes. Everything else in the input, including whitespaces, is kept as it is. See [GRAMMAR.md](./GRAMMAR.md) for the exact grammar that ReparoJSON accepts.


## Commas

A missing comma between elements or members is inserted.

```
$ echo '[1 2]' | reparojson
[1, 2]

$ echo '{"a": 1 "b": 2}' | reparojson
{"a": 1, "b": 2}
```

A trailing comma is removed.

```
$ echo '[1, 2,]' | reparojson
[1, 2]
```

A leading comma is removed.

```
$ echo '[, 1, 2]' | reparojson
[ 1, 2]
```

Duplicate commas are removed.

```
$ echo '[1,, 2]' | reparojson
[1, 2]
```


## Colons

A missing colon between a key and a value is inserted.

```
$ echo '{"a" 1}' | reparojson
{"a": 1}
```


## Strings

A raw control character (U+0000 to U+001F) in a string, such as a tab or a line break, is escaped.

```
$ printf '"a\tb"\n' | reparojson
"a\tb"

$ printf '"a\001b"\n' | reparojson
"a\u0001b"
```


## Numbers

A leading plus sign is removed.

```
$ echo '+1' | reparojson
1
```

Leading zeros are removed.

```
$ echo '007' | reparojson
7
```

A missing integer part is inserted.

```
$ echo '.5' | reparojson
0.5
```


## Unclosed Objects and Arrays

Unclosed objects and arrays at the end of the input are closed.

```
$ printf '[1, {"a": [2,' | reparojson
[1, {"a": [2]}]
```

They are closed only if the input ends where they can be closed: right after `{` or `[`, after a value, or after a comma. An input that ends in the middle of a value (e.g., `["a`) or before a value of an object member (e.g., `{"a":`) is not repaired.


## Byte Order Mark

A UTF-8 byte order mark at the beginning of the input is removed.

```
$ printf '\357\273\277[1]\n' | reparojson
[1]
```
