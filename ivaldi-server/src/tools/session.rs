use crate::state::ServerState;
use crate::tools::ToolError;
use ivaldi_core::session::types::{SessionInitArgs, SessionListArgs, SessionGetArgs, SessionUpdateArgs, Session};
use ivaldi_core::response::IvaldiResponse;
// use serde_json::{json, Value};

pub fn session_init(args: SessionInitArgs, state: &ServerState) -> Result<IvaldiResponse<Session>, ToolError> {
    // 1. Load or Create Session via Manager
    let mut manager = state.session_manager().lock().map_err(|e: std::sync::PoisonError<_>| ToolError::Execution(e.to_string()))?;
    
    // Check if it exists first if auto_create is false?
    // The manager.load_or_create handles creation implicitly if not found.
    // If we want to support strict "attach only", we'd need a separate method on manager.
    // For now, load_or_create is fine. 
    
    let session = manager.load_or_create(&args.id, args.root)
        .map_err(|e: anyhow::Error| ToolError::Execution(e.to_string()))?;
        
    // 2. Update Server State (Context Switch)
    state.set_session(session.clone());
    
    // 3. Return session info
    Ok(IvaldiResponse::success(session))
}

pub fn session_list(_args: SessionListArgs, state: &ServerState) -> Result<IvaldiResponse<Vec<Session>>, ToolError> {
    let manager = state.session_manager().lock().map_err(|e: std::sync::PoisonError<_>| ToolError::Execution(e.to_string()))?;
    let sessions = manager.list();
    Ok(IvaldiResponse::success(sessions))
}

pub fn session_get(args: SessionGetArgs, state: &ServerState) -> Result<IvaldiResponse<Session>, ToolError> {
    // If ID provided, get from manager. If not, get current.
    if let Some(id) = args.id {
        let manager = state.session_manager().lock().map_err(|e: std::sync::PoisonError<_>| ToolError::Execution(e.to_string()))?;
        let session = manager.list().into_iter().find(|s| s.id == id)
            .ok_or_else(|| ToolError::NotFound(format!("Session '{}' not found", id)))?;
        return Ok(IvaldiResponse::success(session));
    }
    
    // Get current
    let current = state.get_session()
        .ok_or_else(|| ToolError::Execution("No active session attached".to_string()))?;
        
    Ok(IvaldiResponse::success(current))
}

pub fn session_update(args: SessionUpdateArgs, state: &ServerState) -> Result<IvaldiResponse<Session>, ToolError> {
    // Must have active session to update (for now, or add ID to args)
    let current = state.get_session()
        .ok_or_else(|| ToolError::Execution("No active session attached to update".to_string()))?;
        
    let mut manager = state.session_manager().lock().map_err(|e: std::sync::PoisonError<_>| ToolError::Execution(e.to_string()))?;
    
    // 1. Retrieve fresh copy from manager (to handle race conditions?)
    // Actually, we trust our current ID.
    // We need to re-fetch from store to modify.
    // But store uses HashMap.
    
    // We can't access store directly here easily without duplicating logic.
    // Let's rely on manager having an update method.
    // Manager has update_session(Session). We need to modify the session struct first.
    
    // We don't have a "get_by_id" on manager exposed public yet except via list() or load_or_create.
    // load_or_create works.
    let mut session = manager.load_or_create(&current.id, None)
        .map_err(|e: anyhow::Error| ToolError::Execution(e.to_string()))?;
        
    // 2. Apply updates
    if let Some(label) = args.label {
        session.metadata.label = Some(label);
    }
    
    if let Some(add_tags) = args.add_tags {
        for tag in add_tags {
            if !session.metadata.tags.contains(&tag) {
                session.metadata.tags.push(tag);
            }
        }
    }
    
    if let Some(remove_tags) = args.remove_tags {
         session.metadata.tags.retain(|t| !remove_tags.contains(t));
    }
    
    // 3. Save
    manager.update_session(session.clone())
        .map_err(|e: anyhow::Error| ToolError::Execution(e.to_string()))?;
        
    // 4. Update local state copy
    state.set_session(session.clone());
    
    Ok(IvaldiResponse::success(session))
}
