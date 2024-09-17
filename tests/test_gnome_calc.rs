//! Automated UI Testing
//!
//! Run with:
//! ```sh
//! cargo test -- --test-threads 1
//! ```

use std::{panic, process::Command, thread::sleep, time::Duration};

use test_by_a11y::prelude::*;

#[cfg(target_os = "linux")]
fn start_test<F>(test_script: F)
where
    F: FnOnce(panic::AssertUnwindSafe<&mut TestByATSPI>) -> () + panic::UnwindSafe,
{
    // Start logging
    let _ = pretty_env_logger::try_init();

    // Start the program
    log::debug!("Starting program...");
    let mut program = Command::new("gnome-calculator")
        .spawn()
        .expect("cannot start child");

    // To allow calculator to start...
    sleep(Duration::from_millis(500));

    // Connect to the accessibility interface
    log::debug!("Connecting to the a11y interface...");
    let result = if let Ok(mut test) = TestByATSPI::connect("gnome-calculator".to_string()) {
        // Run the test, catching any panics
        log::info!("Running test...");
        let wrapper = panic::AssertUnwindSafe(&mut test);
        Some(panic::catch_unwind(move || test_script(wrapper)))
    } else {
        None
    };

    // Kill the program now testing is complete
    log::debug!("Killing child.");
    program.kill().expect("failed to kill child");

    // Resume any panics
    if let Some(result) = result {
        if let Err(e) = result {
            log::debug!("Forwarding panic.");
            panic::resume_unwind(e);
        }
    } else {
        panic!("failed to connect to accessibility interface")
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_ui_1() {
    start_test(|mut test| {
        // Find and click the "9" button
        let btn_9 = test.find(By::Text("9".to_string())).unwrap().unwrap();
        test.interact(&btn_9, Interaction::Click).unwrap();

        // Find and click the "+" button
        let btn_plus = test.find(By::Text("+".to_string())).unwrap().unwrap();
        test.interact(&btn_plus, Interaction::Click).unwrap();

        // Find and click the "1" button
        let btn_1 = test.find(By::Text("1".to_string())).unwrap().unwrap();
        test.interact(&btn_1, Interaction::Click).unwrap();

        // Find and click the "=" button
        let btn_equals = test.find(By::Text("=".to_string())).unwrap().unwrap();
        test.interact(&btn_equals, Interaction::Click).unwrap();

        sleep(Duration::from_millis(100));

        // Check that we find the result "10" written somewhere
        let result = test.find(By::Text("10".to_string())).unwrap();
        assert!(result.is_some());
    });
}

#[test]
#[cfg(target_os = "linux")]
fn test_ui_2() {
    start_test(|mut test| {
        // Find and click the "9" button
        let btn_9 = test.find(By::Text("9".to_string())).unwrap().unwrap();
        test.interact(&btn_9, Interaction::Click).unwrap();

        // Find and click the "+" button
        let btn_plus = test.find(By::Text("+".to_string())).unwrap().unwrap();
        test.interact(&btn_plus, Interaction::Click).unwrap();

        test.interact(&btn_9, Interaction::Click).unwrap();

        // Find and click the "=" button
        let btn_equals = test.find(By::Text("=".to_string())).unwrap().unwrap();
        test.interact(&btn_equals, Interaction::Click).unwrap();

        sleep(Duration::from_millis(100));

        // Check that we find the result "10" written somewhere
        let result = test.find(By::Text("18".to_string())).unwrap();
        assert!(result.is_some());
    });
}
