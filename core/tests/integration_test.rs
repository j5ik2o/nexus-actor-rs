use nexus_actor_core_rs::actor::process_registry::*;
use nexus_actor_core_rs::actor::pid::Pid;
use std::time::Duration;

#[tokio::test]
async fn test_process_registry_integration() {
    // Setup
    let registry = ProcessRegistry::new("test-host".to_string());
    let pid_cache = PidCache::new(Duration::from_secs(1));
    let member_list = MemberList::new();
    let remote_registry = RemoteProcessRegistry::new();

    // Test member registration and process handling
    member_list.add_member("node-1".to_string(), "address-1".to_string());
    
    let pid = Pid {
        id: Some("test-1".to_string()),
        ..Default::default()
    };

    remote_registry.register(pid.clone(), "address-1".to_string()).await;
    pid_cache.add("test-key".to_string(), pid.clone(), "address-1".to_string());

    // Verify integration
    assert!(member_list.get_member("node-1").is_some());
    assert!(remote_registry.get_process(&pid).is_some());
    assert!(pid_cache.get("test-key").is_some());

    // Test member removal and cleanup
    member_list.remove_member("node-1");
    remote_registry.remove(&pid);
    
    assert!(member_list.get_member("node-1").is_none());
    assert!(remote_registry.get_process(&pid).is_none());
}
