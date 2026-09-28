//! Desktop notifications must be a silent no-op when there is no session bus
//! (SSH, headless CI, air-gapped machine) and when disabled.
//!
//! Its own test binary: it points `DBUS_SESSION_BUS_ADDRESS` at a socket
//! that doesn't exist, which must not leak into other tests' processes.

use starlab_client::elm::desktop_notify;

#[cfg(target_os = "linux")]
#[test]
fn no_session_bus_is_an_error_not_a_panic_and_disabled_sends_nothing() {
    // SAFETY: the only test in this binary; set before any other thread
    // (the notify thread below) reads the environment.
    unsafe {
        std::env::set_var(
            "DBUS_SESSION_BUS_ADDRESS",
            "unix:path=/nonexistent/starlab-test-bus",
        );
    }

    assert!(desktop_notify::show_blocking("Starlab test", "no bus").is_err());

    // Enabled: dispatched off-thread; that thread logs the failure and exits.
    let worker = desktop_notify::send(true, "Starlab test", "no bus");
    assert!(worker.is_some());
    assert!(
        worker.unwrap().join().is_ok(),
        "notify thread must not panic without a bus"
    );

    // Disabled: nothing is dispatched at all.
    assert!(desktop_notify::send(false, "Starlab test", "disabled").is_none());
}
