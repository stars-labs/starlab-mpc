//! Performance monitoring and profiling utilities

use std::time::{Duration, Instant};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Performance metrics for tracking operation timings
#[derive(Debug, Clone)]
pub struct PerformanceMetrics {
    pub operation: String,
    pub duration: Duration,
    pub timestamp: Instant,
}

/// Performance monitor for tracking and analyzing application performance
pub struct PerformanceMonitor {
    metrics: Arc<Mutex<HashMap<String, Vec<PerformanceMetrics>>>>,
    enabled: bool,
}

impl Default for PerformanceMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl PerformanceMonitor {
    /// Create a new performance monitor
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(Mutex::new(HashMap::new())),
            enabled: std::env::var("PERF_MONITORING").is_ok(),
        }
    }
    
    /// Start timing an operation
    pub fn start_timer(&self) -> OperationTimer {
        OperationTimer {
            start: Instant::now(),
            monitor: self.clone(),
        }
    }
    
    /// Record a metric
    pub async fn record_metric(&self, operation: String, duration: Duration) {
        if !self.enabled {
            return;
        }
        
        let metric = PerformanceMetrics {
            operation: operation.clone(),
            duration,
            timestamp: Instant::now(),
        };
        
        let mut metrics = self.metrics.lock().await;
        let op_clone = operation.clone();
        metrics
            .entry(operation)
            .or_insert_with(Vec::new)
            .push(metric);
        
        // Keep only last 1000 entries per operation
        if let Some(vec) = metrics.get_mut(&op_clone)
            && vec.len() > 1000 {
                vec.drain(0..vec.len() - 1000);
            }
    }
    
    /// Get performance summary
    pub async fn get_summary(&self) -> String {
        let metrics = self.metrics.lock().await;
        let mut summary = String::from("=== Performance Summary ===\n");
        
        for (operation, measurements) in metrics.iter() {
            if measurements.is_empty() {
                continue;
            }
            
            let total: Duration = measurements.iter().map(|m| m.duration).sum();
            let avg = total / measurements.len() as u32;
            let min = measurements.iter().map(|m| m.duration).min().unwrap_or(Duration::ZERO);
            let max = measurements.iter().map(|m| m.duration).max().unwrap_or(Duration::ZERO);
            
            summary.push_str(&format!(
                "{}: count={}, avg={:?}, min={:?}, max={:?}\n",
                operation,
                measurements.len(),
                avg,
                min,
                max
            ));
        }
        
        summary
    }
    
    /// Clear all metrics
    pub async fn clear(&self) {
        let mut metrics = self.metrics.lock().await;
        metrics.clear();
    }
}

impl Clone for PerformanceMonitor {
    fn clone(&self) -> Self {
        Self {
            metrics: self.metrics.clone(),
            enabled: self.enabled,
        }
    }
}

/// Timer for tracking operation duration
pub struct OperationTimer {
    start: Instant,
    monitor: PerformanceMonitor,
}

impl OperationTimer {
    /// Stop the timer and record the metric
    pub async fn stop(self, operation: &str) {
        let duration = self.start.elapsed();
        self.monitor.record_metric(operation.to_string(), duration).await;
        
        // Log slow operations
        if duration > Duration::from_millis(100) {
            tracing::warn!("Slow operation '{}': {:?}", operation, duration);
        }
    }
}

/// Macro for timing a block of code
#[macro_export]
macro_rules! time_operation {
    ($monitor:expr, $op_name:expr, $code:block) => {{
        let timer = $monitor.start_timer();
        let result = $code;
        timer.stop($op_name).await;
        result
    }};
}

/// Profile key event handling performance
pub async fn profile_key_handling(event_type: &str, duration: Duration) {
    if duration > Duration::from_millis(50) {
        tracing::warn!(
            "Slow key event handling for '{}': {:?}ms",
            event_type,
            duration.as_millis()
        );
    }
}

/// Profile render performance
pub async fn profile_render(duration: Duration) {
    if duration > Duration::from_millis(16) {  // 60 FPS = 16.67ms per frame
        tracing::debug!(
            "Render took {:?}ms (target: 16ms for 60 FPS)",
            duration.as_millis()
        );
    }
}

/// Profile DKG operation
pub async fn profile_dkg_operation(phase: &str, duration: Duration) {
    tracing::info!(
        "DKG {} completed in {:?}ms",
        phase,
        duration.as_millis()
    );
}

/// Performance optimization recommendations
pub fn analyze_performance(_monitor: &PerformanceMonitor) -> Vec<String> {
    vec![
        // Render performance
        "Enable render throttling to prevent excessive redraws".to_string(),
        "Use cached values for addresses to prevent regeneration".to_string(),
        "Batch UI updates to reduce render calls".to_string(),
        // Key handling
        "Debounce rapid key events to prevent UI lag".to_string(),
        "Process key events asynchronously where possible".to_string(),
        // DKG performance
        "Use deterministic session IDs for consistent address generation".to_string(),
        "Cache group public keys to prevent recalculation".to_string(),
    ]
}

