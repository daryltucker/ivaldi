//! Tier 2 Tests: Anchor Trimming with Whitespace Mismatch
//!
//! **THE BUG**: Agents include surrounding context "anchors" but with different indentation
//! than the target file. The current anchor_trimming uses exact trim equality,
//! which fails when whitespace differs between agent replacement and file content.
//!
//! This causes: code duplication when agent tries to do surgical edits.

use ivaldi_core::ast_edit::{edit_content, EditSelector};
use vecq::FileType;

#[tokio::test]
#[allow(non_snake_case)]
async fn test_anchor_trimming_exact_match() {
    // Baseline: current behavior (should work)
    let content = "line1\nline2\nline3\nline4\nline5";
    let selector = EditSelector::Lines(3, 3);
    let replacement = "line2\nNEW_LINE3\nline4";  // Agent includes anchors EXACTLY as in file
    
    let outcome = edit_content(content, FileType::Text, selector, replacement).await.unwrap();
    
    // Overlaps trimmed correctly
    assert_eq!(outcome.content, "line1\nline2\nNEW_LINE3\nline4\nline5");
    assert!(outcome.heuristics_triggered.contains(&"anchor_trimming_leading".to_string()));
    assert!(outcome.heuristics_triggered.contains(&"anchor_trimming_trailing".to_string()));
}

#[tokio::test]
#[allow(non_snake_case)]
async fn test_anchor_trimming_whitespace_mismatch_causes_duplication() {
    // **THE BUG REPRODUCTION** - Now fixed!
    // File has indented content
    let content = "line1\n    line2\n    line3\n    line4\n    line5";
    let selector = EditSelector::Lines(3, 3);  // Target line3
    
    // Agent sends replacement with anchors BUT different indentation
    let replacement = "    line2\nNEW_LINE3\n    line4";
    
    let outcome = edit_content(content, FileType::Text, selector, replacement).await.unwrap();
    
    // **AFTER FIX**: This should trigger BOTH trimming heuristics
    // "line2".trim() == "line2".trim() is TRUE ✓
    // "line4".trim() == "line4".trim() is TRUE ✓
    
    let has_trimming = outcome.heuristics_triggered.iter().any(|h| 
        h.contains("anchor_trimming")
    );
    
    assert!(has_trimming, "Anchor trimming should trigger even with whitespace differences");
    
    // Verify no duplication - each anchor appears once
    assert!(!outcome.content.contains("line2\nline2"), "Leading anchor should be trimmed");
    assert!(!outcome.content.contains("line4\nline4"), "Trailing anchor should be trimmed");
}

#[tokio::test]
async fn test_anchor_trimming_leading_only_with_whitespace() {
    // Test case: leading anchor matches (with whitespace) but trailing doesn't
    // This tests that trimmed comparison works for leading anchor
    let content = "    line1\n    line2\n    line3\n    line4";
    let selector = EditSelector::Lines(2, 3);  // Replace lines 2-3
    
    // Agent includes CORRECT anchor ("line1") but with different indentation
    // The leading anchor matches lines[0] (after trim) so it should be trimmed
    let replacement = "line1\nNEW_LINES\nDIFFERENT";
    
    let outcome = edit_content(content, FileType::Text, selector, replacement).await.unwrap();
    
    // With the fix, leading should be trimmed because "line1" == lines[0].trim()
    // After indentation healing: ["    line1", "    NEW_LINES", "    DIFFERENT"]
    // Anchor trimming: "line1".trim() == "line1".trim() → TRUE!
    let has_leading_trim = outcome.heuristics_triggered.contains(&"anchor_trimming_leading".to_string());
    
    if !has_leading_trim {
        println!("BUG: Leading anchor not trimmed despite matching trimmed content");
    }
    
    // Verify the fix works
    assert!(has_leading_trim, "Leading anchor should be trimmed when it matches (after trim)");
}

#[tokio::test]
async fn test_anchor_trimming_trailing_only_with_whitespace() {
    // Test case: trailing anchor matches (with whitespace) but leading doesn't
    // This tests that trimmed comparison works for trailing anchor
    let content = "    line1\n    line2\n    line3\n    line4";
    let selector = EditSelector::Lines(2, 3);  // Replace lines 2-3
    
    // Agent includes CORRECT anchor ("line4") but with different indentation
    let replacement = "DIFFERENT\nNEW_LINES\nline4";
    
    let outcome = edit_content(content, FileType::Text, selector, replacement).await.unwrap();
    
    // With the fix, trailing should be trimmed because "line4" == lines[3].trim()
    let has_trailing_trim = outcome.heuristics_triggered.contains(&"anchor_trimming_trailing".to_string());
    
    if !has_trailing_trim {
        println!("BUG: Trailing anchor not trimmed despite matching trimmed content");
    }
    
    // Verify the fix works
    assert!(has_trailing_trim, "Trailing anchor should be trimmed when it matches (after trim)");
}

#[tokio::test]
async fn test_no_trim_for_unrelated_content() {
    // Edge case: replacement contains similar but UNRELATED content (not anchors)
    // Should NOT trigger trimming
    let content = "line1\n    line2\n    line3\n    line4";
    let selector = EditSelector::Lines(2, 2);
    
    // This is NOT an anchor - it's different content that happens to be similar
    let replacement = "    UNRELATED_CONTENT\n    line2";  // line2 appears but as DIFFERENT content
    
    let outcome = edit_content(content, FileType::Text, selector, replacement).await.unwrap();
    
    // Should NOT trigger trimming heuristics (different content, not anchors)
    let has_any_trim = outcome.heuristics_triggered.iter().any(|h| h.contains("anchor_trimming"));
    
    // This is correct - we should NOT trim unrelated content
    assert!(!has_any_trim, "Should not trim unrelated content that happens to be similar");
}