use super::*;
use std::time::Duration;

#[tokio::test]
async fn test_process_registry() {
    let registry = ProcessRegistry::new("test-host".to_string());
    let pid = Pid {
        id: Some("test-1".to_string()),
        ..Default::default()
    };
    
    // Test process registration
    registry.add("test-1".to_string(), Arc::new(RemoteProcess::new(pid.clone(), "test-address".to_string())));
    assert!(registry.get(&pid).is_some());
    
    // Test process removal
    registry.remove(&pid);
    assert!(registry.get(&pid).is_none());
}

#[tokio::test]
async fn test_pid_cache() {
    let cache = PidCache::new(Duration::from_secs(1));
    let pid = Pid {
        id: Some("test-1".to_string()),
        ..Default::default()
    };
    
    // Test cache addition
    cache.add("test-key".to_string(), pid.clone(), "test-address".to_string());
    assert!(cache.get("test-key").is_some());
    
    // Test cache expiration
    tokio::time::sleep(Duration::from_secs(2)).await;
    assert!(cache.get("test-key").is_none());
}

#[tokio::test]
async fn test_member_list() {
    let member_list = MemberList::new();
    
    // Test member addition
    member_list.add_member("test-1".to_string(), "test-address".to_string());
    assert!(member_list.get_member("test-1").is_some());
    
    // Test member status update
    member_list.update_member_status("test-1", MemberStatus::Leaving);
    let member = member_list.get_member("test-1").unwrap();
    assert_eq!(member.status, MemberStatus::Leaving);
    
    // Test member removal
    member_list.remove_member("test-1");
    assert!(member_list.get_member("test-1").is_none());
}
