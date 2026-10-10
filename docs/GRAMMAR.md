# Grammar

## JSON

The original JSON grammar, taken from <https://www.json.org/json-en.html>.

```
json
    element

value
    object
    array
    string
    number
    "true"
    "false"
    "null"

object
    '{' ws '}'
    '{' members '}'

members
    member
    member ',' members

member
    ws string ws ':' element

array
    '[' ws ']'
    '[' elements ']'

elements
    element
    element ',' elements

element
    ws value ws

string
    '"' characters '"'

characters
    ""
    character characters

character
    '0020' . '10FFFF' - '"' - '\'
    '\' escape

escape
    '"'
    '\'
    '/'
    'b'
    'f'
    'n'
    'r'
    't'
    'u' hex hex hex hex

hex
    digit
    'A' . 'F'
    'a' . 'f'

number
    integer fraction exponent

integer
    digit
    onenine digits
    '-' digit
    '-' onenine digits

digits
    digit
    digit digits

digit
    '0'
    onenine

onenine
    '1' . '9'

fraction
    ""
    '.' digits

exponent
    ""
    'E' sign digits
    'e' sign digits

sign
    ""
    '+'
    '-'

ws
    ""
    '0020' ws
    '000A' ws
    '000D' ws
    '0009' ws
```


## JSON that ReparoJSON accepts

ReparoJSON accepts a superset of JSON.

```
json
    bom element

bom
    ""
    'FEFF'

object
    '{' ws leading_comma '}'
    '{' ws leading_comma members trailing_commas '}'

members
    member ws
    member ws separator members

member
    string ws colon ws value

colon
    ""
    ':'

array
    '[' ws leading_comma ']'
    '[' ws leading_comma elements trailing_commas ']'

elements
    value ws
    value ws separator elements

leading_comma
    ""
    ',' ws

separator
    ""
    commas

trailing_commas
    ""
    commas

commas
    ',' ws
    ',' ws commas

character
    '0000' . '10FFFF' - '"' - '\'
    '\' escape

number
    plus minus digits fraction exponent
    plus minus '.' digits exponent

plus
    ""
    '+'

minus
    ""
    '-'
```

* In `elements`, `separator` cannot be empty if `value` on its left is a number, `"true"`, `"false"` or `"null"`, `ws` between them is empty, and `value` on its right does not begin with `'"'`, `'['` or `'{'`. For example, `[1 2]` and `[1"a"]` are accepted, but `[1+2]` and `[truefalse]` are not.
* Any number of `'}'` and `']'` at the end of the input can be omitted. That is, an input is also accepted if it matches the grammar after `'}'` and `']'` are appended to it. For example, `[1, {"a": [2,` is accepted because `[1, {"a": [2,]}]` matches the grammar.
* The input is processed as a sequence of bytes and is not validated as UTF-8. `bom` is the three bytes `EF BB BF`, and any byte other than `'"'` and `'\'` is accepted as `character`.
* There is no limit on the nesting depth of objects and arrays, the length of a string, or the magnitude and the number of digits of a number. Numbers are not interpreted, so a number that does not fit in any numeric type (e.g., `1e999999999999`) is accepted as is.

See [REPAIR.md](./REPAIR.md) for how the accepted input is repaired.
