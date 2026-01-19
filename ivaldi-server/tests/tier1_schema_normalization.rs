//! Schema Normalization Regression Tests
//!
//! Verifies that build-time schema normalization produces clean JSON Schema
//! that works with all LLM providers (especially strict validators like Gemini).

/// Test that the runtime_manual.json embedded in the server has no extended schema fields.
#[test]
fn test_schema_no_extended_fields() {
    // Load the embedded runtime manual (same one served via tools/list)
    const MANUAL_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/runtime_manual.json"));
    
    let manual: serde_json::Value = serde_json::from_str(MANUAL_JSON)
        .expect("runtime_manual.json should be valid JSON");
    
    let tools = manual["tools"].as_array()
        .expect("Should have tools array");
    
    assert!(!tools.is_empty(), "Should have at least one tool");
    
    for tool in tools {
        let name = tool["name"].as_str().unwrap_or("unknown");
        let schema = &tool["inputSchema"];
        
        // Recursively check for problematic fields
        check_no_extended_fields(schema, &format!("tool:{}", name));
    }
}

/// Recursively check a schema for extended fields that strict validators reject.
fn check_no_extended_fields(schema: &serde_json::Value, context: &str) {
    if let Some(obj) = schema.as_object() {
        // 1. No format on integer types (uint, uint64, int32, etc.)
        if obj.get("type").map(|t| {
            t.as_str() == Some("integer") || 
            t.as_array().map(|arr| arr.iter().any(|v| v.as_str() == Some("integer"))).unwrap_or(false)
        }).unwrap_or(false) {
            assert!(
                !obj.contains_key("format"),
                "{}: integer type should not have 'format' field", context
            );
        }
        
        // 2. No float minimum (0.0 instead of 0)
        if let Some(min) = obj.get("minimum") {
            if let Some(f) = min.as_f64() {
                // Check it's a clean integer, not a float
                assert!(
                    f.fract() == 0.0 && min.is_i64(),
                    "{}: minimum should be integer, not float: {}", context, min
                );
            }
        }
        
        // 3. No $schema meta field in tool inputSchema
        assert!(
            !obj.contains_key("$schema"),
            "{}: should not have $schema meta field", context
        );

        // 4. No nullable type arrays ["type", "null"] -> should be "type"
        if let Some(type_val) = obj.get("type") {
            if let Some(arr) = type_val.as_array() {
                // We strictly forbid ["type", "null"] because Gemini rejects it
                let has_null = arr.iter().any(|v| v.as_str() == Some("null"));
                assert!(
                    !has_null,
                    "{}: type should be a string (e.g., 'string'), not an array with null (found {:?}). Auto-flattener failed.", 
                    context, arr
                );
            }
        }

        // 5. Objects MUST have additionalProperties: false
        if obj.get("type").map(|t| t.as_str() == Some("object")).unwrap_or(false) {
            assert_eq!(
                obj.get("additionalProperties"),
                Some(&serde_json::json!(false)),
                "{}: object type MUST have 'additionalProperties: false' for strict mode compliance", 
                context
            );
        }
        
        // Recurse into properties
        if let Some(props) = obj.get("properties") {
            if let Some(props_obj) = props.as_object() {
                for (prop_name, prop_schema) in props_obj {
                    check_no_extended_fields(prop_schema, &format!("{}:{}", context, prop_name));
                }
            }
        }
        
        // Recurse into definitions
        if let Some(defs) = obj.get("definitions") {
            if let Some(defs_obj) = defs.as_object() {
                for (def_name, def_schema) in defs_obj {
                    check_no_extended_fields(def_schema, &format!("{}:def:{}", context, def_name));
                }
            }
        }
        
        // Recurse into anyOf/oneOf/allOf
        for key in ["anyOf", "oneOf", "allOf"] {
            if let Some(arr) = obj.get(key) {
                if let Some(arr_vec) = arr.as_array() {
                    for (i, item) in arr_vec.iter().enumerate() {
                        check_no_extended_fields(item, &format!("{}:{}[{}]", context, key, i));
                    }
                }
            }
        }
        
        // Recurse into items (array types)
        if let Some(items) = obj.get("items") {
            check_no_extended_fields(items, &format!("{}:items", context));
        }
    }
}

/// Test specific tools have clean schemas
#[test]
fn test_find_files_schema_is_clean() {
    const MANUAL_JSON: &str = include_str!(concat!(env!("OUT_DIR"), "/runtime_manual.json"));
    let manual: serde_json::Value = serde_json::from_str(MANUAL_JSON).unwrap();
    
    let find_files = manual["tools"].as_array().unwrap()
        .iter()
        .find(|t| t["name"] == "find_files")
        .expect("Should have find_files tool");
    
    let schema = &find_files["inputSchema"];
    let props = schema["properties"].as_object().unwrap();
    
    // max_depth should be clean integer, no format
    let max_depth = &props["max_depth"];
    assert_eq!(max_depth["type"], "integer");
    assert!(!max_depth.as_object().unwrap().contains_key("format"), 
        "max_depth should not have format field");
    
    // timeout_ms should also be clean
    let timeout = &props["timeout_ms"];
    assert_eq!(timeout["type"], "integer");
    assert!(!timeout.as_object().unwrap().contains_key("format"),
        "timeout_ms should not have format field");
}
