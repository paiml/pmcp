//! Tests for server-side roots support.

use pmcp::Server;

#[tokio::test]
async fn test_server_roots_registration() {
    let server = Server::builder()
        .name("test-server")
        .version("1.0.0")
        .build()
        .unwrap();

    // Register some roots
    let _unregister1 = server
        .register_root("file:///home/user/project1", Some("Project 1".to_string()))
        .await
        .unwrap();

    let _unregister2 = server
        .register_root("file:///home/user/project2", None)
        .await
        .unwrap();

    // Get roots
    let roots = server.get_roots().await;
    assert_eq!(roots.len(), 2);
    assert_eq!(roots[0].uri, "file:///home/user/project1");
    assert_eq!(roots[0].name, Some("Project 1".to_string()));
    assert_eq!(roots[1].uri, "file:///home/user/project2");
    assert_eq!(roots[1].name, None);
}

#[tokio::test]
async fn test_server_roots_invalid_uri() {
    let server = Server::builder()
        .name("test-server")
        .version("1.0.0")
        .build()
        .unwrap();

    // Try to register a non-file URI
    let result = server
        .register_root("https://example.com/project", None)
        .await;

    assert!(result.is_err());
    // Check the error without unwrapping
    match result {
        Err(e) => assert!(e.to_string().contains("must start with file://")),
        Ok(_) => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_server_roots_unregister() {
    let server = Server::builder()
        .name("test-server")
        .version("1.0.0")
        .build()
        .unwrap();

    // Register a root
    let unregister = server
        .register_root("file:///home/user/project", None)
        .await
        .unwrap();

    // Check it's registered
    let roots = server.get_roots().await;
    assert_eq!(roots.len(), 1);

    // Unregister
    unregister();

    // Give the async unregister time to complete
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    // Check it's unregistered
    let roots = server.get_roots().await;
    assert_eq!(roots.len(), 0);
}
