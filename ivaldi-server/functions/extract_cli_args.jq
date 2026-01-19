# Extract CLI arguments from Rust clap Args struct
# Usage: vecq src/cli.rs -f functions/extract_cli_args.jq
#
# Output format:
# [
#   {
#     "name": "conversation_id",
#     "env": "IVALDI_CONVERSATION_ID",
#     "flag": "--conversation-id",
#     "description": "Conversation ID for naked/stdio drivers",
#     "default": "None"
#   },
#   ...
# ]

.structs[]
| select(.name == "Args")
| .content
| split("\n")
| . as $lines
| reduce range(0; length) as $i (
    [];
    if $lines[$i] | test("^\\s*///") then
      # Found a doc comment
      . + [{
        description: ($lines[$i] | gsub("^\\s*///\\s*"; "")),
        line: $i
      }]
    else
      .
    end
  )
| map(
    . as $doc
    | $lines[$doc.line + 1] as $attr_line
    | $lines[$doc.line + 2] as $field_line
    | if ($attr_line | test("#\\[arg\\(")) then
        {
          description: $doc.description,
          name: ($field_line | gsub("^\\s*pub\\s+"; "") | gsub(":.*$"; "") | gsub("\\s+"; "")),
          env: (
            if ($attr_line | test("env\\s*=\\s*\"")) then
              $attr_line | gsub(".*env\\s*=\\s*\""; "") | gsub("\".*"; "")
            else
              ""
            end
          ),
          flag: (
            if ($attr_line | test("long")) then
              "--" + ($field_line | gsub("^\\s*pub\\s+"; "") | gsub(":.*$"; "") | gsub("\\s+"; ""))
            else
              ""
            end
          ),
          default: "None"
        }
      else
        empty
      end
  )
