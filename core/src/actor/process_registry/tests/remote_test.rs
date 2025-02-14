use super::*;

#[tokio::test]
async fn test_remote_process_registry() {
    let registry = RemoteProcessRegistry::new();
    let pid = Pid {
        id: Some("test-remote-1".to_string()),
        ..Default::default()
    };
    
    // Test process registration
    registry.register(pid.clone(), "remote-address".to_string()).await;
    let process = registry.get_process(&pid);
    assert!(process.is_some());
    assert_eq!(process.unwrap().get_address(), "remote-address");
    
    // Test process removal
    registry.remove(&pid);
    assert!(registry.get_process(&pid).is_none());
    
    // Test availability status
    assert!(registry.is_available().await);
    registry.set_available(false).await;
    assert!(!registry.is_available().await);
}
