

/// Normalize JSON Schema for universal LLM provider compatibility.
/// 
/// Removes/transforms extended fields that strict validators reject:
/// - `format` on integer types (uint, uint64, int32, etc.)
/// - `minimum` as float (0.0 → 0)
/// - `$schema` meta field (not needed for tool definitions)
/// - Adds `additionalProperties: false` to all objects
/// 
/// This is applied at build time to produce clean schemas that work with
/// all providers (Gemini, Claude, OpenAI, local models).
pub fn normalize_schema(schema: &mut serde_json::Value) {
    if let Some(obj) = schema.as_object_mut() {
        // Remove $schema meta field (not needed in tool inputSchema)
        obj.remove("$schema");
        
        // Remove format annotations on integer types
        // These are schemars extensions (uint, uint64, int32, etc.) that strict validators reject
        if obj.get("type").map(|t| {
            t.as_str() == Some("integer") || 
            t.as_array().map(|arr| arr.iter().any(|v| v.as_str() == Some("integer"))).unwrap_or(false)
        }).unwrap_or(false) {
            obj.remove("format");
        }

        // Flatten nullable types: ["type", "null"] -> "type"
        // Strict validators often prefer simple types for optional fields (which are just omitted if null)
        if let Some(type_val) = obj.get_mut("type") && let Some(arr) = type_val.as_array() && arr.len() == 2 && arr.iter().any(|v| v.as_str() == Some("null")) {
            // Find the non-null type
            if let Some(real_type) = arr.iter().find(|v| v.as_str() != Some("null")) {
                *type_val = real_type.clone();
            }
        }
        
        // Convert float minimum to integer (0.0 → 0)
        if let Some(min) = obj.get_mut("minimum") && let Some(f) = min.as_f64() {
             *min = serde_json::json!(f as i64);
        }
        
        // Enforce strictness: add additionalProperties: false to all objects
        if obj.get("type").map(|t| t.as_str() == Some("object")).unwrap_or(false) {
            // Only add if not already present (though unlikely to be present with true)
            if !obj.contains_key("additionalProperties") {
                obj.insert("additionalProperties".to_string(), json!(false));
            }

            // Ensure 'properties' exists (even if empty)
            if !obj.contains_key("properties") {
                obj.insert("properties".to_string(), json!({}));
            }

            // Ensure 'required' exists (even if empty)
            if !obj.contains_key("required") {
                obj.insert("required".to_string(), json!([]));
            }
        }
        
        // Recurse into properties
        if let Some(props) = obj.get_mut("properties") && let Some(props_obj) = props.as_object_mut() {
            for (_, prop_schema) in props_obj.iter_mut() {
                normalize_schema(prop_schema);
            }
        }
        
        // Recurse into definitions (for complex nested types)
        if let Some(defs) = obj.get_mut("definitions") && let Some(defs_obj) = defs.as_object_mut() {
            for (_, def_schema) in defs_obj.iter_mut() {
                normalize_schema(def_schema);
            }
        }
        
        // Recurse into anyOf/oneOf (for Option<T> types)
        for key in ["allOf", "anyOf", "oneOf"] {
            if let Some(arr) = obj.get_mut(key) && let Some(arr_vec) = arr.as_array_mut() {
                for item in arr_vec {
                    normalize_schema(item);
                }
            }
        }
        
        // Recurse into items (for array types)
        if let Some(items) = obj.get_mut("items") {
            normalize_schema(items);
        }
    }
}
