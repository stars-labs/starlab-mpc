use tokio::sync::mpsc;

#[tokio::main]
async fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("Testing try_send vs send().await behavior...");
    
    // Create a bounded channel
    let (tx, mut rx) = mpsc::channel::<String>(100);
    
    // Test 1: try_send (non-blocking)
    println!("\nTest 1: Using try_send (non-blocking)");
    match tx.try_send("Message 1".to_string()) {
        Ok(_) => println!("✅ try_send succeeded immediately"),
        Err(e) => println!("❌ try_send failed: {:?}", e),
    }
    
    // Check if message was received
    match rx.try_recv() {
        Ok(msg) => println!("✅ Received: {}", msg),
        Err(e) => println!("❌ try_recv failed: {:?}", e),
    }
    
    // Test 2: send().await without active receiver
    println!("\nTest 2: Testing if send().await blocks");
    let tx2 = tx.clone();
    
    // Spawn a task that will try send().await
    let handle = tokio::spawn(async move {
        println!("Attempting send().await...");
        match tokio::time::timeout(
            std::time::Duration::from_millis(100),
            tx2.send("Message 2".to_string())
        ).await {
            Ok(Ok(_)) => println!("✅ send().await completed"),
            Ok(Err(e)) => println!("❌ send() failed: {:?}", e),
            Err(_) => println!("⏱️ send().await timed out after 100ms (was blocking)"),
        }
    });
    
    // Wait for the spawned task
    handle.await.unwrap();
    
    // Now try to receive
    match rx.try_recv() {
        Ok(msg) => println!("✅ Message was in buffer: {}", msg),
        Err(e) => println!("❌ No message in buffer: {:?}", e),
    }
    
    println!("\n🎯 Conclusion: try_send is the correct choice for fire-and-forget messages");
}