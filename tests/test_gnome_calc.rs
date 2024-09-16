use std::{panic, process::Command};

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
    std::thread::sleep(std::time::Duration::from_millis(500));

    // Connect to the accessibility interface
    log::debug!("Connecting to the a11y interface...");
    let mut test = TestByATSPI::connect("gnome-calculator".to_string())
        .expect("failed to connect to accessibility interface");

    // Run the test, catching any panics
    log::info!("Running test...");
    let wrapper = panic::AssertUnwindSafe(&mut test);
    let result = panic::catch_unwind(move || test_script(wrapper));

    // Kill the program now testing is complete
    log::debug!("Killing child.");
    program.kill().expect("failed to kill child");

    // Resume any panics
    if let Err(e) = result {
        log::debug!("Forwarding panic.");
        panic::resume_unwind(e);
    }
}

#[test]
#[cfg(target_os = "linux")]
fn test_ui() {
    start_test(|mut test| {
        // Find and click the "9" button
        let btn_9 = test.find(By::Text("9".to_string())).unwrap().unwrap();
        test.interact(btn_9, Interaction::Click).unwrap();

        // Find and click the "+" button
        let btn_plus = test.find(By::Text("+".to_string())).unwrap().unwrap();
        test.interact(btn_plus, Interaction::Click).unwrap();

        // Find and click the "1" button
        let btn_1 = test.find(By::Text("1".to_string())).unwrap().unwrap();
        test.interact(btn_1, Interaction::Click).unwrap();

        // Find and click the "=" button
        let btn_equals = test.find(By::Text("=".to_string())).unwrap().unwrap();
        test.interact(btn_equals, Interaction::Click).unwrap();

        // Check that we find the result "10" written somewhere
        let result = test.find(By::Text("10".to_string())).unwrap();
        assert!(result.is_some());
    });
}
