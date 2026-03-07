

/// Normalize JSON Schema for universal LLM provider compatibility.
/// 
/// Removes/transforms extended fields that strict validators reject:
/// - `format` on integer types (uint, uint64, int32, etc.)
/// - `minimum` as float (0.0 → 0)
/// - `$schema` meta field (not needed for tool definitions)
/// - Adds `additionalProperties: false` to all objects
/// - **Flattens top-level oneOf/allOf/anyOf** (OpenCode doesn't support these)
/// 
/// This is applied at build time to produce clean schemas that work with
/// all providers (Gemini, Claude, OpenAI, OpenCode, local models).
pub fn normalize_schema(schema: &mut serde_json::Value) {
    // First, flatten top-level oneOf/allOf/anyOf (before other normalizations)
    flatten_top_level_union(schema);

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

/// Flatten top-level oneOf/allOf/anyOf into a single object schema.
/// 
/// OpenCode (and some other providers) don't support union types at the top level.
/// This function merges all variants into a single object with all properties,
/// making each property optional (since it may not be required in all variants).
/// 
/// The `action` field is preserved as an enum of all possible values.
fn flatten_top_level_union(schema: &mut serde_json::Value) {
    if let Some(obj) = schema.as_object_mut() {
        // Check for oneOf, allOf, or anyOf at top level
        for union_key in ["oneOf", "allOf", "anyOf"] {
            if let Some(union_val) = obj.remove(union_key) {
                if let Some(variants) = union_val.as_array() {
                    // Merge all variants into combined properties
                    let mut all_properties: serde_json::Map<String, serde_json::Value> = serde_json::Map::new();
                    let mut action_enum_values: Vec<String> = Vec::new();
                    
                    for variant in variants {
                        if let Some(variant_obj) = variant.as_object() {
                            // Extract action enum value from this variant
                            if let Some(props) = variant_obj.get("properties").and_then(|p| p.as_object()) {
                                if let Some(enum_vals) = props.get("action").and_then(|p| p.get("enum")).and_then(|e| e.as_array()) {
                                    for val in enum_vals {
                                        if let Some(s) = val.as_str()
                                            && !action_enum_values.contains(&s.to_string()) {
                                            action_enum_values.push(s.to_string());
                                        }
                                    }
                                }
                                
                                // Merge all properties from this variant
                                for (prop_name, prop_schema) in props {
                                    if prop_name != "action" {
                                        // Only add if not already present (first definition wins)
                                        if !all_properties.contains_key(prop_name) {
                                            all_properties.insert(prop_name.clone(), prop_schema.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    
                    // Bail out if no "action" enum values were found (not a polymorphic action object)
                    if action_enum_values.is_empty() {
                        obj.insert(union_key.to_string(), json!(variants));
                        return;
                    }
                    
                    // Build the flattened schema
                    // Get existing properties or create empty object
                    let properties = obj.entry("properties")
                        .or_insert_with(|| json!({}))
                        .as_object_mut()
                        .unwrap();
                    
                    // Add action property with combined enum
                    if !action_enum_values.is_empty() {
                        properties.insert("action".to_string(), json!({
                            "type": "string",
                            "enum": action_enum_values,
                            "description": "The action to perform"
                        }));
                    }
                    
                    // Add all merged properties
                    for (prop_name, prop_schema) in all_properties {
                        if !properties.contains_key(&prop_name) {
                            properties.insert(prop_name, prop_schema);
                        }
                    }
                    
                    // Update required to only require "action" (other fields depend on action)
                    obj.insert("required".to_string(), json!(["action"]));
                    
                    // Ensure it's marked as object type
                    obj.insert("type".to_string(), json!("object"));
                    obj.insert("additionalProperties".to_string(), json!(false));
                }
                
                // Only process one union type (oneOf takes precedence)
                break;
            }
        }
    }
}
